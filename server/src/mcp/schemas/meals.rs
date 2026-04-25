//! Meal-related MCP input/output types, conversions, and resolver helpers.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::dto::{CreateMealDto, PersonServingDto};
use crate::entities::meal;

use crate::mcp::lookups::MealLookups;

use super::common::{
    ingredient_in, ingredient_out, parse_json, validate_date_yyyy_mm_dd, IngredientOut,
};
use super::errors::{CreateMealError, InputError, ResolveError, VALID_MEAL_TYPES};

/// One serving within a meal. The `kind` discriminator distinguishes a recipe
/// assignment from an ad-hoc item list.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServingOut {
    Recipe {
        person_name: String,
        recipe_slug: String,
        recipe_name: String,
        servings_count: f64,
        notes: Option<String>,
    },
    Adhoc {
        person_name: String,
        items: Vec<IngredientOut>,
        notes: Option<String>,
    },
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MealBrief {
    pub id: String,
    pub date: String,
    pub meal_type: String,
    pub order_index: i32,
    pub servings: Vec<ServingOut>,
}

/// Input for `create_meal`. Uses `person_name` and `recipe_slug` instead of
/// the underlying UUIDs — the tool resolves them.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateMealInput {
    /// Date of the meal in YYYY-MM-DD format.
    pub date: String,
    /// One of "breakfast", "lunch", "dinner", or "snack".
    pub meal_type: String,
    /// Slot position within the day. When omitted, defaults to the
    /// canonical slot for the meal_type (Breakfast=0, Lunch=1, Dinner=2,
    /// Snack=3) so the meal renders in the expected planner slot. Only
    /// override when scheduling a secondary/tertiary meal of the same type
    /// on the same date (e.g. "second dinner").
    #[serde(default)]
    pub order_index: Option<i32>,
    pub servings: Vec<ServingInput>,
}

/// One serving assignment within a meal. `kind = "recipe"` references an
/// existing recipe; `kind = "adhoc"` carries a loose ingredient list for
/// people who are eating something off-menu.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServingInput {
    Recipe {
        /// Must match an active family member's name (case-insensitive).
        person_name: String,
        /// Must match an existing recipe's slug.
        recipe_slug: String,
        /// How many recipe-servings this person is eating (e.g. 1.0 for a
        /// standard serving, 0.5 for half). The shopping-list aggregator
        /// scales ingredients by this factor.
        servings_count: f64,
        #[serde(default)]
        notes: Option<String>,
    },
    Adhoc {
        person_name: String,
        /// Ingredients the person is eating without a named recipe.
        items: Vec<IngredientOut>,
        #[serde(default)]
        notes: Option<String>,
    },
}

// ─── Read side: meal → MealBrief ────────────────────────────────

pub fn meal_to_brief(meal: &meal::Model, lookups: &MealLookups) -> Result<MealBrief, String> {
    let servings: Vec<PersonServingDto> = parse_json(&meal.servings, "meal servings")?;
    let servings_out = servings
        .into_iter()
        .map(|s| serving_out(&meal.id, s, lookups))
        .collect::<Vec<_>>();
    Ok(MealBrief {
        id: meal.id.clone(),
        date: meal.date.format("%Y-%m-%d").to_string(),
        meal_type: meal.meal_type.clone(),
        order_index: meal.order_index,
        servings: servings_out,
    })
}

/// Convert a [`PersonServingDto`] into its LLM-facing form. Dangling
/// references to soft-deleted people or deleted recipes are surfaced with a
/// clearly-unreal placeholder (so the LLM won't try to round-trip the
/// placeholder back through `get_recipe`) and logged as a warning so
/// operators can see referential-integrity drift.
fn serving_out(meal_id: &str, s: PersonServingDto, lookups: &MealLookups) -> ServingOut {
    match s {
        PersonServingDto::Recipe {
            person_id,
            recipe_id,
            servings_count,
            notes,
        } => {
            let person_name = resolve_person_name(meal_id, &person_id, lookups);
            let (recipe_slug, recipe_name) = resolve_recipe_info(meal_id, &recipe_id, lookups);
            ServingOut::Recipe {
                person_name,
                recipe_slug,
                recipe_name,
                servings_count,
                notes,
            }
        }
        PersonServingDto::Adhoc {
            person_id,
            adhoc_items,
            notes,
        } => {
            let person_name = resolve_person_name(meal_id, &person_id, lookups);
            ServingOut::Adhoc {
                person_name,
                items: adhoc_items.iter().map(ingredient_out).collect(),
                notes,
            }
        }
    }
}

fn resolve_person_name(meal_id: &str, person_id: &str, lookups: &MealLookups) -> String {
    match lookups.person_display_name(person_id) {
        Some(name) => name.to_string(),
        None => {
            tracing::warn!(
                meal_id,
                person_id,
                "meal references a person that isn't in the active-family map \
                 (likely soft-deleted). Surfacing as placeholder."
            );
            // A parenthetical sentinel — not a valid name, so the LLM
            // won't attempt to reuse it as an identifier.
            format!("(inactive person, id={person_id})")
        }
    }
}

fn resolve_recipe_info(meal_id: &str, recipe_id: &str, lookups: &MealLookups) -> (String, String) {
    match lookups.recipe_display(recipe_id) {
        Some((slug, name)) => (slug.to_string(), name.to_string()),
        None => {
            tracing::warn!(
                meal_id,
                recipe_id,
                "meal references a recipe that no longer exists. \
                 Surfacing as placeholder."
            );
            (
                "(unknown)".to_string(),
                format!("(deleted recipe, id={recipe_id})"),
            )
        }
    }
}

// ─── Write side: CreateMealInput → CreateMealDto ────────────────

pub fn create_meal_input_to_dto(
    input: CreateMealInput,
    lookups: &MealLookups,
) -> Result<CreateMealDto, CreateMealError> {
    validate_date_yyyy_mm_dd(&input.date, "date")?;

    let meal_type = canonical_meal_type(&input.meal_type)
        .ok_or_else(|| InputError::UnknownMealType(input.meal_type.clone()))?;

    for s in &input.servings {
        if let ServingInput::Recipe { servings_count, .. } = s {
            if !servings_count.is_finite() || *servings_count <= 0.0 {
                return Err(InputError::NonPositiveServingsCount(*servings_count).into());
            }
        }
    }

    let order_index = input
        .order_index
        .unwrap_or_else(|| default_order_index(&meal_type));

    let servings = input
        .servings
        .into_iter()
        .map(|s| serving_input_to_dto(s, lookups))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CreateMealDto {
        date: input.date,
        meal_type,
        order_index,
        servings,
    })
}

/// Default `order_index` for a given canonical meal_type, matching the UI's
/// `DEFAULT_MEALS` slot mapping in `src/components/MealPlanner.tsx`. The
/// planner renders per-day slots by doing strict `meal_type === X &&
/// order_index === Y` equality, so a Dinner at order 0 would silently not
/// appear in any slot even though it's in the data. Snack has no slot in
/// the current UI — it gets 3 as a stable non-colliding default.
fn default_order_index(canonical_meal_type: &str) -> i32 {
    match canonical_meal_type {
        "Breakfast" => 0,
        "Lunch" => 1,
        "Dinner" => 2,
        "Snack" => 3,
        _ => 0,
    }
}

/// Normalize an LLM-supplied meal_type to the canonical Title-Case form the
/// rest of fewd uses. Accepts any case (`"dinner"`, `"DINNER"`, `"Dinner"`)
/// plus leading/trailing whitespace. Returns `None` for unknown types so the
/// caller can surface a user-facing error.
///
/// Why this matters: the web UI in `MealPlanner.tsx` renders per-day cells
/// by strict equality on `meal_type` (`meal.meal_type === 'Dinner'`), so any
/// MCP-created meal stored as `"dinner"` silently disappears from the
/// planner even though it's visible in the shopping-list aggregation.
pub(super) fn canonical_meal_type(input: &str) -> Option<String> {
    let trimmed = input.trim().to_lowercase();
    VALID_MEAL_TYPES
        .iter()
        .find(|canonical| canonical.to_lowercase() == trimmed)
        .map(|s| s.to_string())
}

fn serving_input_to_dto(
    serving: ServingInput,
    lookups: &MealLookups,
) -> Result<PersonServingDto, ResolveError> {
    match serving {
        ServingInput::Recipe {
            person_name,
            recipe_slug,
            servings_count,
            notes,
        } => {
            let person_id = resolve_person(&person_name, lookups)?;
            let recipe_id = resolve_recipe(&recipe_slug, lookups)?;
            Ok(PersonServingDto::Recipe {
                person_id,
                recipe_id,
                servings_count,
                notes,
            })
        }
        ServingInput::Adhoc {
            person_name,
            items,
            notes,
        } => {
            let person_id = resolve_person(&person_name, lookups)?;
            Ok(PersonServingDto::Adhoc {
                person_id,
                adhoc_items: items.into_iter().map(ingredient_in).collect(),
                notes,
            })
        }
    }
}

fn resolve_person(name: &str, lookups: &MealLookups) -> Result<String, ResolveError> {
    lookups
        .person_id_for_name(name)
        .map(|s| s.to_string())
        .ok_or_else(|| ResolveError::UnknownPerson(name.to_string()))
}

fn resolve_recipe(slug: &str, lookups: &MealLookups) -> Result<String, ResolveError> {
    lookups
        .recipe_id_for_slug(slug)
        .map(|s| s.to_string())
        .ok_or_else(|| ResolveError::UnknownRecipe(slug.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};

    fn mk_lookups() -> MealLookups {
        MealLookups::from_people_and_recipes(
            vec![("pid".into(), "Alice".into())],
            vec![("rid".into(), "carbonara".into(), "Carbonara".into())],
        )
    }

    #[test]
    fn create_meal_input_defaults_order_index_to_ui_slot_for_meal_type() {
        // The MealPlanner UI renders per-day slots by strict equality on
        // both meal_type AND order_index (DEFAULT_MEALS in MealPlanner.tsx).
        // Default order_index must match the slot the LLM would expect for
        // each canonical meal_type.
        for (meal_type, expected_order) in
            [("breakfast", 0), ("lunch", 1), ("dinner", 2), ("snack", 3)]
        {
            let input = CreateMealInput {
                date: "2026-04-22".into(),
                meal_type: meal_type.into(),
                order_index: None,
                servings: vec![],
            };
            let dto = create_meal_input_to_dto(input, &mk_lookups()).unwrap();
            assert_eq!(
                dto.order_index, expected_order,
                "{meal_type} should default to order_index {expected_order}"
            );
        }
    }

    #[test]
    fn create_meal_input_respects_explicit_order_index() {
        // If the LLM passes order_index explicitly, we honor it rather than
        // overriding with the meal-type default.
        let input = CreateMealInput {
            date: "2026-04-22".into(),
            meal_type: "dinner".into(),
            order_index: Some(7),
            servings: vec![ServingInput::Recipe {
                person_name: "Alice".into(),
                recipe_slug: "carbonara".into(),
                servings_count: 1.0,
                notes: None,
            }],
        };
        let dto = create_meal_input_to_dto(input, &mk_lookups()).unwrap();
        assert_eq!(dto.order_index, 7);
        assert_eq!(dto.servings.len(), 1);
    }

    #[test]
    fn create_meal_input_reports_unknown_recipe_slug_with_helpful_message() {
        let input = CreateMealInput {
            date: "2026-04-22".into(),
            meal_type: "dinner".into(),
            order_index: Some(0),
            servings: vec![ServingInput::Recipe {
                person_name: "Alice".into(),
                recipe_slug: "ghost-recipe".into(),
                servings_count: 1.0,
                notes: None,
            }],
        };
        let err = create_meal_input_to_dto(input, &mk_lookups()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("ghost-recipe"));
        assert!(msg.contains("search_recipes"));
    }

    #[test]
    fn create_meal_input_reports_unknown_person_name_with_helpful_message() {
        let input = CreateMealInput {
            date: "2026-04-22".into(),
            meal_type: "dinner".into(),
            order_index: None,
            servings: vec![ServingInput::Recipe {
                person_name: "Nobody".into(),
                recipe_slug: "carbonara".into(),
                servings_count: 1.0,
                notes: None,
            }],
        };
        let err = create_meal_input_to_dto(input, &mk_lookups()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Nobody"));
        assert!(msg.contains("list_people"));
    }

    #[test]
    fn create_meal_input_rejects_malformed_date() {
        let input = CreateMealInput {
            date: "not-a-date".into(),
            meal_type: "dinner".into(),
            order_index: None,
            servings: vec![],
        };
        let err = create_meal_input_to_dto(input, &mk_lookups()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("date"));
        assert!(msg.contains("YYYY-MM-DD"));
        assert!(msg.contains("not-a-date"));
    }

    #[test]
    fn create_meal_input_accepts_valid_date() {
        let input = CreateMealInput {
            date: "2026-04-26".into(),
            meal_type: "dinner".into(),
            order_index: None,
            servings: vec![],
        };
        assert!(create_meal_input_to_dto(input, &mk_lookups()).is_ok());
    }

    #[test]
    fn create_meal_input_rejects_unknown_meal_type() {
        let input = CreateMealInput {
            date: "2026-04-22".into(),
            meal_type: "brunch".into(),
            order_index: None,
            servings: vec![],
        };
        let err = create_meal_input_to_dto(input, &mk_lookups()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("brunch"));
        assert!(msg.contains("Breakfast"));
    }

    #[test]
    fn create_meal_input_rejects_non_positive_servings_count() {
        let input = CreateMealInput {
            date: "2026-04-22".into(),
            meal_type: "dinner".into(),
            order_index: None,
            servings: vec![ServingInput::Recipe {
                person_name: "Alice".into(),
                recipe_slug: "carbonara".into(),
                servings_count: -1.0,
                notes: None,
            }],
        };
        let err = create_meal_input_to_dto(input, &mk_lookups()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("servings_count"));
    }

    #[test]
    fn create_meal_input_rejects_non_finite_servings_count() {
        let input = CreateMealInput {
            date: "2026-04-22".into(),
            meal_type: "dinner".into(),
            order_index: None,
            servings: vec![ServingInput::Recipe {
                person_name: "Alice".into(),
                recipe_slug: "carbonara".into(),
                servings_count: f64::NAN,
                notes: None,
            }],
        };
        assert!(create_meal_input_to_dto(input, &mk_lookups()).is_err());
    }

    #[test]
    fn create_meal_input_normalizes_meal_type_to_title_case() {
        // Title Case matches the web UI convention (MealPlanner.tsx does
        // strict `meal_type === 'Dinner'` equality).
        for variant in ["dinner", "DINNER", "Dinner", "  Dinner  ", "dINnEr"] {
            let input = CreateMealInput {
                date: "2026-04-22".into(),
                meal_type: variant.into(),
                order_index: None,
                servings: vec![],
            };
            let dto = create_meal_input_to_dto(input, &mk_lookups()).unwrap();
            assert_eq!(
                dto.meal_type, "Dinner",
                "expected {variant:?} to normalize to 'Dinner'"
            );
        }
    }

    #[test]
    fn create_meal_input_normalizes_all_four_meal_types() {
        for (input_value, expected) in [
            ("breakfast", "Breakfast"),
            ("lunch", "Lunch"),
            ("dinner", "Dinner"),
            ("snack", "Snack"),
        ] {
            let input = CreateMealInput {
                date: "2026-04-22".into(),
                meal_type: input_value.into(),
                order_index: None,
                servings: vec![],
            };
            let dto = create_meal_input_to_dto(input, &mk_lookups()).unwrap();
            assert_eq!(dto.meal_type, expected);
        }
    }

    #[test]
    fn meal_to_brief_surfaces_dangling_recipe_with_unreal_slug() {
        let meal = meal::Model {
            id: "meal-1".into(),
            date: NaiveDate::from_ymd_opt(2026, 4, 24).unwrap(),
            meal_type: "dinner".into(),
            order_index: 0,
            servings: serde_json::to_string(&[PersonServingDto::Recipe {
                person_id: "alice-id".into(),
                recipe_id: "dangling-recipe-id".into(),
                servings_count: 1.0,
                notes: None,
            }])
            .unwrap(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        // Lookups know about Alice but NOT the dangling recipe id.
        let lookups =
            MealLookups::from_people_and_recipes(vec![("alice-id".into(), "Alice".into())], vec![]);

        let brief = meal_to_brief(&meal, &lookups).unwrap();
        assert_eq!(brief.servings.len(), 1);
        match &brief.servings[0] {
            ServingOut::Recipe {
                person_name,
                recipe_slug,
                recipe_name,
                ..
            } => {
                assert_eq!(person_name, "Alice");
                // Placeholder is clearly-unreal; the LLM won't try to
                // round-trip it through get_recipe.
                assert_eq!(recipe_slug, "(unknown)");
                assert!(recipe_name.contains("dangling-recipe-id"));
            }
            _ => panic!("expected Recipe serving"),
        }
    }

    #[test]
    fn meal_to_brief_surfaces_dangling_person_with_placeholder() {
        let meal = meal::Model {
            id: "meal-2".into(),
            date: NaiveDate::from_ymd_opt(2026, 4, 24).unwrap(),
            meal_type: "dinner".into(),
            order_index: 0,
            servings: serde_json::to_string(&[PersonServingDto::Recipe {
                person_id: "ghost".into(),
                recipe_id: "rid".into(),
                servings_count: 1.0,
                notes: None,
            }])
            .unwrap(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let lookups = MealLookups::from_people_and_recipes(
            vec![],
            vec![("rid".into(), "carbonara".into(), "Carbonara".into())],
        );
        let brief = meal_to_brief(&meal, &lookups).unwrap();
        match &brief.servings[0] {
            ServingOut::Recipe { person_name, .. } => {
                assert!(person_name.contains("inactive"));
                assert!(person_name.contains("ghost"));
            }
            _ => panic!("expected Recipe serving"),
        }
    }
}
