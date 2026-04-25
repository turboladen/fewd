//! Identifier-translation tables shared between the read and write meal
//! tools. MCP exposes recipes and people by human-readable identifiers (slug,
//! name), but the DB stores UUIDs; these tables translate in both directions
//! without issuing N+1 queries across servings.

use std::collections::HashMap;

use sea_orm::DatabaseConnection;

use crate::services::person_service::PersonService;
use crate::services::recipe_service::RecipeService;

/// Lookup tables built once per meal tool call.
pub(super) struct MealLookups {
    /// person_id → display name.
    person_names: HashMap<String, String>,
    /// recipe_id → (slug, display name).
    recipe_info: HashMap<String, (String, String)>,
    /// Lowercased person name → person_id.
    person_id_by_name: HashMap<String, String>,
    /// Lowercased recipe slug → recipe_id.
    recipe_id_by_slug: HashMap<String, String>,
}

impl MealLookups {
    /// Construct a lookup directly from in-memory vectors. Used by
    /// sibling-module tests that need a MealLookups without a database.
    /// Goes through the same ambiguity-detecting path as [`load`] so test
    /// behavior matches production.
    #[cfg(test)]
    pub(super) fn from_people_and_recipes(
        people: Vec<(String, String)>,          // (id, display name)
        recipes: Vec<(String, String, String)>, // (id, slug, display name)
    ) -> Self {
        let person_id_by_name = warn_and_collect_unambiguous(
            people
                .iter()
                .map(|(id, name)| (name.trim().to_lowercase(), id.clone())),
            "person_name",
        );
        let recipe_id_by_slug = warn_and_collect_unambiguous(
            recipes
                .iter()
                .map(|(id, slug, _name)| (slug.trim().to_lowercase(), id.clone())),
            "recipe_slug",
        );
        let person_names = people.into_iter().collect();
        let recipe_info = recipes
            .into_iter()
            .map(|(id, slug, name)| (id, (slug, name)))
            .collect();
        Self {
            person_names,
            recipe_info,
            person_id_by_name,
            recipe_id_by_slug,
        }
    }

    pub(super) async fn load(db: &DatabaseConnection) -> Result<Self, sea_orm::DbErr> {
        let people = PersonService::get_all(db).await?;
        let recipes = RecipeService::get_all(db).await?;

        let person_id_by_name = warn_and_collect_unambiguous(
            people
                .iter()
                .map(|p| (p.name.trim().to_lowercase(), p.id.clone())),
            "person_name",
        );
        let recipe_id_by_slug = warn_and_collect_unambiguous(
            recipes
                .iter()
                .map(|r| (r.slug.trim().to_lowercase(), r.id.clone())),
            "recipe_slug",
        );
        let person_names = people.into_iter().map(|p| (p.id, p.name)).collect();
        let recipe_info = recipes
            .into_iter()
            .map(|r| (r.id, (r.slug, r.name)))
            .collect();
        Ok(Self {
            person_names,
            recipe_info,
            person_id_by_name,
            recipe_id_by_slug,
        })
    }

    /// Display name for a person by id, or `None` if the person has been
    /// deactivated since the meal was scheduled.
    pub(super) fn person_display_name(&self, id: &str) -> Option<&str> {
        self.person_names.get(id).map(String::as_str)
    }

    /// (slug, display name) for a recipe by id, or `None` if the recipe
    /// was deleted since the meal was scheduled.
    pub(super) fn recipe_display(&self, id: &str) -> Option<(&str, &str)> {
        self.recipe_info
            .get(id)
            .map(|(slug, name)| (slug.as_str(), name.as_str()))
    }

    /// Resolve a human-provided person name (case- and whitespace-insensitive)
    /// to its UUID, or `None` if no active family member matches.
    pub(super) fn person_id_for_name(&self, name: &str) -> Option<&str> {
        self.person_id_by_name
            .get(name.trim().to_lowercase().as_str())
            .map(String::as_str)
    }

    /// Resolve a recipe slug (case- and whitespace-insensitive) to its UUID,
    /// or `None` if no recipe matches.
    pub(super) fn recipe_id_for_slug(&self, slug: &str) -> Option<&str> {
        self.recipe_id_by_slug
            .get(slug.trim().to_lowercase().as_str())
            .map(String::as_str)
    }
}

/// Collect `(normalized_key, id)` pairs into a `HashMap`, dropping any keys
/// that two-or-more different ids resolve to. Logs a `tracing::warn!` for
/// each ambiguous key so the operator can see + clean up the data.
///
/// Mirrors the fail-closed posture of `PersonService::find_active_by_name`:
/// when the data is ambiguous, we'd rather refuse to resolve at all than
/// silently pick whichever entry happened to win the `HashMap` overwrite
/// race. Callers (`person_id_for_name`, `recipe_id_for_slug`) will then
/// return `None`, and the meal-write path surfaces that as a clear
/// `invalid_params` error pointing the LLM at the discovery tool.
fn warn_and_collect_unambiguous(
    pairs: impl IntoIterator<Item = (String, String)>,
    kind: &'static str,
) -> HashMap<String, String> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for (k, v) in pairs {
        groups.entry(k).or_default().push(v);
    }
    groups
        .into_iter()
        .filter_map(|(key, ids)| match ids.len() {
            1 => Some((key, ids.into_iter().next().expect("len() == 1"))),
            n => {
                tracing::warn!(
                    kind,
                    normalized_key = %key,
                    matched_ids = ?ids,
                    match_count = n,
                    "MealLookups: ambiguous {kind}; refusing to resolve at meal-time. \
                     Edit one of the duplicates so they're distinguishable."
                );
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::MealLookups;
    use std::collections::HashMap;

    fn mk_lookups() -> MealLookups {
        let mut person_names = HashMap::new();
        person_names.insert("p1".into(), "Alice".into());
        let mut recipe_info = HashMap::new();
        recipe_info.insert(
            "r1".into(),
            ("carbonara".to_string(), "Carbonara".to_string()),
        );
        let mut person_id_by_name = HashMap::new();
        person_id_by_name.insert("alice".into(), "p1".into());
        let mut recipe_id_by_slug = HashMap::new();
        recipe_id_by_slug.insert("carbonara".into(), "r1".into());
        MealLookups {
            person_names,
            recipe_info,
            person_id_by_name,
            recipe_id_by_slug,
        }
    }

    #[test]
    fn person_id_for_name_is_case_insensitive() {
        let l = mk_lookups();
        assert_eq!(l.person_id_for_name("Alice"), Some("p1"));
        assert_eq!(l.person_id_for_name("  ALICE  "), Some("p1"));
        assert_eq!(l.person_id_for_name("Bob"), None);
    }

    #[test]
    fn recipe_id_for_slug_is_case_insensitive() {
        let l = mk_lookups();
        assert_eq!(l.recipe_id_for_slug("Carbonara"), Some("r1"));
        assert_eq!(l.recipe_id_for_slug("carbonara"), Some("r1"));
        assert_eq!(l.recipe_id_for_slug("ghost"), None);
    }

    #[test]
    fn from_people_and_recipes_drops_ambiguous_person_names() {
        // Two different person ids whose names normalize to the same key.
        // Both display-name lookups should still work (id → name is fine),
        // but the reverse name → id lookup must refuse to resolve.
        let lookups = MealLookups::from_people_and_recipes(
            vec![
                ("p1".into(), "Alice".into()),
                ("p2".into(), "  ALICE  ".into()),
            ],
            vec![],
        );
        assert_eq!(lookups.person_display_name("p1"), Some("Alice"));
        assert_eq!(lookups.person_display_name("p2"), Some("  ALICE  "));
        assert_eq!(
            lookups.person_id_for_name("alice"),
            None,
            "ambiguous person_name must not resolve"
        );
    }

    #[test]
    fn from_people_and_recipes_drops_ambiguous_recipe_slugs() {
        let lookups = MealLookups::from_people_and_recipes(
            vec![],
            vec![
                ("r1".into(), "carbonara".into(), "Carbonara".into()),
                ("r2".into(), "  CARBONARA  ".into(), "Carbonara".into()),
            ],
        );
        assert_eq!(
            lookups.recipe_id_for_slug("carbonara"),
            None,
            "ambiguous recipe_slug must not resolve"
        );
    }

    #[test]
    fn from_people_and_recipes_keeps_unambiguous_entries() {
        // Make sure the ambiguity-detection doesn't drop cleanly distinct rows.
        let lookups = MealLookups::from_people_and_recipes(
            vec![("p1".into(), "Alice".into()), ("p2".into(), "Bob".into())],
            vec![],
        );
        assert_eq!(lookups.person_id_for_name("Alice"), Some("p1"));
        assert_eq!(lookups.person_id_for_name("Bob"), Some("p2"));
    }

    #[test]
    fn display_lookups_return_none_for_unknown_ids() {
        let l = mk_lookups();
        assert_eq!(l.person_display_name("p1"), Some("Alice"));
        assert_eq!(l.person_display_name("missing"), None);
        assert_eq!(l.recipe_display("r1"), Some(("carbonara", "Carbonara")));
        assert_eq!(l.recipe_display("missing"), None);
    }
}
