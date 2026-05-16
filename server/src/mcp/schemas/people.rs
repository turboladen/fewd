//! People-related MCP input/output types, conversions, and the Markdown
//! renderer backing the `fewd://family/overview` resource.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::dto::UpdatePersonDto;
use crate::entities::person;

use super::common::{parse_json, parse_optional_json};

#[derive(Debug, Serialize, JsonSchema)]
pub struct PersonWithPrefs {
    pub name: String,
    pub dietary_goals: Option<String>,
    pub dislikes: Vec<String>,
    pub favorites: Vec<String>,
    pub notes: Option<String>,
    pub drink_preferences: Vec<String>,
    pub drink_dislikes: Vec<String>,
}

pub fn person_to_prefs(person: &person::Model) -> Result<PersonWithPrefs, String> {
    let dislikes: Vec<String> = parse_json(&person.dislikes, "person dislikes")?;
    let favorites: Vec<String> = parse_json(&person.favorites, "person favorites")?;
    // The `drink_*` columns are nullable in the schema (unlike `dislikes` /
    // `favorites` which are NOT NULL with a `[]` default). Treat a NULL
    // column as an empty list at the MCP boundary so the JSON Schema can
    // expose a uniform `Vec<String>` and the renderer below doesn't need
    // a third "field never set" state alongside "empty list" and
    // "populated list". The write path can still distinguish them — but
    // the read path doesn't need to.
    let drink_preferences: Vec<String> = parse_optional_json(
        person.drink_preferences.as_deref(),
        "person drink_preferences",
    )?
    .unwrap_or_default();
    let drink_dislikes: Vec<String> =
        parse_optional_json(person.drink_dislikes.as_deref(), "person drink_dislikes")?
            .unwrap_or_default();
    Ok(PersonWithPrefs {
        name: person.name.clone(),
        dietary_goals: person.dietary_goals.clone(),
        dislikes,
        favorites,
        notes: person.notes.clone(),
        drink_preferences,
        drink_dislikes,
    })
}

/// Input for the `update_person` MCP tool. `name` identifies the row to
/// update (case-insensitive, mirroring `list_people` / `create_meal`
/// resolution). Every other field is optional with PATCH semantics: an
/// omitted field — or an explicit JSON `null` — leaves the column
/// unchanged.
///
/// **Clear semantics differ by field shape**:
///
/// - `notes` (free-form string column): there is NO clear-to-NULL path
///   for this column anywhere in fewd today — not via this tool, not via
///   the web UI (which sends `notes: formData.notes || undefined` and
///   so cannot clear either). `PersonService::update`'s convention is
///   "set" only, never "clear". To preserve that invariant at the MCP
///   boundary, `update_person_input_to_dto` normalizes an empty or
///   whitespace-only `notes` string to `None` — without that
///   normalization a caller could persist `Some("")` as a back-door
///   clear (the renderer collapses it to `_none_`), silently forking
///   the codebase invariant. If a real "clear" affordance is wanted
///   later, it has to land in `UpdatePersonDto` + service + UI
///   together so all three paths agree.
/// - The four list fields (`dislikes`, `favorites`, `drink_preferences`,
///   `drink_dislikes`): passing `[]` REPLACES the existing list with an
///   empty array. That's the same write path as setting them to any other
///   list — `Some(vec![])` flows through `update_person_input_to_dto` →
///   `Set(to_json(&vec![]))` → `"[]"` in the DB. Reasonable for the
///   "clean-up bad data" use case; deliberate that we don't coalesce
///   empty arrays to "no-op" because that would silently drop a
///   legitimate write.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdatePersonInput {
    pub name: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub dislikes: Option<Vec<String>>,
    #[serde(default)]
    pub favorites: Option<Vec<String>>,
    #[serde(default)]
    pub drink_preferences: Option<Vec<String>>,
    #[serde(default)]
    pub drink_dislikes: Option<Vec<String>>,
}

/// Translate `UpdatePersonInput` into the existing `UpdatePersonDto` the
/// service layer accepts. The `name` on the input is the lookup key, NOT a
/// write — the DTO's `name` (which would rename the person) is always left
/// `None` because renames touch other tables that reference person.name and
/// belong in a separate, more deliberate tool.
///
/// `notes` gets one normalization step: empty / whitespace-only input is
/// coerced to `None` so the codebase invariant "no clear-to-NULL path for
/// notes" holds at the MCP boundary. See `UpdatePersonInput`'s docstring
/// for the full rationale.
pub fn update_person_input_to_dto(input: UpdatePersonInput) -> UpdatePersonDto {
    UpdatePersonDto {
        name: None,
        birthdate: None,
        dietary_goals: None,
        is_active: None,
        notes: input
            .notes
            .and_then(|s| if s.trim().is_empty() { None } else { Some(s) }),
        dislikes: input.dislikes,
        favorites: input.favorites,
        drink_preferences: input.drink_preferences,
        drink_dislikes: input.drink_dislikes,
    }
}

/// Render active family members as Markdown for the `fewd://family/overview`
/// resource (and its tool mirror `get_family_overview`). Keeps every person's
/// dietary goals, likes, dislikes, notes, and drink preferences in one place
/// so AI clients that auto-load resources have immediate context.
///
/// Every person's block always emits all six bullets in the same order so
/// the reader can tell "empty field" from "field not rendered": an empty
/// list, `None`, or whitespace-only value becomes `_none_` rather
/// than a missing line.
pub fn render_family_overview(people: &[person::Model]) -> Result<String, String> {
    let mut out = String::from("# Family overview\n\n");
    if people.is_empty() {
        out.push_str("_No active family members recorded yet._\n");
        return Ok(out);
    }

    for p in people {
        let prefs = person_to_prefs(p)?;
        out.push_str(&format!("## {}\n\n", prefs.name));
        out.push_str(&format!(
            "- **Dietary goals**: {}\n",
            optional_string(prefs.dietary_goals.as_deref())
        ));
        out.push_str(&format!(
            "- **Dislikes**: {}\n",
            list_or_none(&prefs.dislikes)
        ));
        out.push_str(&format!(
            "- **Favorites**: {}\n",
            list_or_none(&prefs.favorites)
        ));
        out.push_str(&format!(
            "- **Notes**: {}\n",
            optional_string(prefs.notes.as_deref())
        ));
        out.push_str(&format!(
            "- **Drink preferences**: {}\n",
            list_or_none(&prefs.drink_preferences)
        ));
        out.push_str(&format!(
            "- **Drink dislikes**: {}\n",
            list_or_none(&prefs.drink_dislikes)
        ));
        out.push('\n');
    }
    Ok(out)
}

const NONE_MARKER: &str = "_none_";

fn list_or_none(items: &[String]) -> String {
    if items.is_empty() {
        NONE_MARKER.to_string()
    } else {
        items.join(", ")
    }
}

fn optional_string(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => NONE_MARKER.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};

    fn mk_person(name: &str) -> person::Model {
        person::Model {
            id: format!("id-{name}"),
            name: name.to_string(),
            birthdate: NaiveDate::from_ymd_opt(1990, 1, 1).unwrap(),
            dietary_goals: Some("low-carb".into()),
            dislikes: "[\"olives\",\"beets\"]".into(),
            favorites: "[\"pasta\"]".into(),
            notes: Some("picky about onions".into()),
            drink_preferences: Some("[\"whiskey neat\",\"tonic + lime\"]".into()),
            drink_dislikes: Some("[\"gin\"]".into()),
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            mcp_token_hash: None,
            mcp_token_fingerprint: None,
        }
    }

    #[test]
    fn family_overview_renders_markdown() {
        let out = render_family_overview(&[mk_person("Alice"), mk_person("Bob")]).unwrap();
        assert!(out.starts_with("# Family overview"));
        assert!(out.contains("## Alice"));
        assert!(out.contains("## Bob"));
        assert!(out.contains("**Dietary goals**: low-carb"));
        assert!(out.contains("**Dislikes**: olives, beets"));
        assert!(out.contains("**Favorites**: pasta"));
        assert!(out.contains("**Notes**: picky about onions"));
        assert!(out.contains("**Drink preferences**: whiskey neat, tonic + lime"));
        assert!(out.contains("**Drink dislikes**: gin"));
    }

    #[test]
    fn family_overview_with_no_people_is_explicit() {
        let out = render_family_overview(&[]).unwrap();
        assert!(out.contains("No active family members"));
    }

    #[test]
    fn family_overview_always_renders_every_bullet_even_when_empty() {
        // Every person section must emit all six bullets in the same
        // order so an "empty field" can't be mistaken for "not rendered".
        let mut p = mk_person("Vivienne");
        p.dietary_goals = None;
        p.dislikes = "[]".into();
        p.favorites = "[]".into();
        p.notes = None;
        p.drink_preferences = None;
        p.drink_dislikes = Some("[]".into());

        let out = render_family_overview(&[p]).unwrap();
        assert!(out.contains("## Vivienne"));
        assert!(out.contains("**Dietary goals**: _none_"));
        assert!(out.contains("**Dislikes**: _none_"));
        assert!(out.contains("**Favorites**: _none_"));
        assert!(out.contains("**Notes**: _none_"));
        // NULL column and empty-array column both collapse to `_none_` at
        // the MCP boundary — the read surface treats "field never set" and
        // "field explicitly empty" identically.
        assert!(out.contains("**Drink preferences**: _none_"));
        assert!(out.contains("**Drink dislikes**: _none_"));
    }

    #[test]
    fn family_overview_treats_whitespace_only_fields_as_empty() {
        // A dietary_goals of "   " shouldn't be rendered as-is — that's
        // a data-entry artifact, not a meaningful value.
        let mut p = mk_person("Whitespace");
        p.dietary_goals = Some("   ".into());
        p.notes = Some("\t\n".into());

        let out = render_family_overview(&[p]).unwrap();
        assert!(out.contains("**Dietary goals**: _none_"));
        assert!(out.contains("**Notes**: _none_"));
    }

    #[test]
    fn person_to_prefs_parses_json_arrays() {
        let prefs = person_to_prefs(&mk_person("Alice")).unwrap();
        assert_eq!(prefs.name, "Alice");
        assert_eq!(prefs.dislikes, vec!["olives", "beets"]);
        assert_eq!(prefs.favorites, vec!["pasta"]);
        assert_eq!(
            prefs.drink_preferences,
            vec!["whiskey neat", "tonic + lime"]
        );
        assert_eq!(prefs.drink_dislikes, vec!["gin"]);
    }

    #[test]
    fn person_to_prefs_null_drink_columns_decode_as_empty_vec() {
        // The schema permits NULL in drink_preferences / drink_dislikes,
        // and `person_to_prefs` collapses NULL → empty list so JSON Schema
        // can expose a non-nullable `Vec<String>`.
        let mut p = mk_person("Sober");
        p.drink_preferences = None;
        p.drink_dislikes = None;
        let prefs = person_to_prefs(&p).unwrap();
        assert!(prefs.drink_preferences.is_empty());
        assert!(prefs.drink_dislikes.is_empty());
    }

    #[test]
    fn person_to_prefs_rejects_malformed_json() {
        let mut p = mk_person("Broken");
        p.dislikes = "not-json".into();
        let err = person_to_prefs(&p).unwrap_err();
        assert!(err.contains("person dislikes"));
    }

    #[test]
    fn person_to_prefs_rejects_malformed_drink_json() {
        // Same diagnostic path as the food-side `dislikes` malformed-JSON
        // case, just routed through `parse_optional_json` (since the
        // column is `Option<String>`).
        let mut p = mk_person("Broken");
        p.drink_preferences = Some("not-json".into());
        let err = person_to_prefs(&p).unwrap_err();
        assert!(
            err.contains("person drink_preferences"),
            "error must name the offending field: {err}"
        );
    }

    #[test]
    fn update_person_input_to_dto_drops_the_lookup_name_and_forwards_writable_fields() {
        // `name` on the input is the lookup key — it must NOT propagate
        // into `UpdatePersonDto.name` (which would rename the row).
        let input = UpdatePersonInput {
            name: "Alice".into(),
            notes: Some("needs smaller portion".into()),
            dislikes: None,
            favorites: Some(vec!["mac and cheese".into()]),
            drink_preferences: None,
            drink_dislikes: None,
        };
        let dto = update_person_input_to_dto(input);
        assert!(
            dto.name.is_none(),
            "lookup name must NOT become a rename: {:?}",
            dto.name
        );
        // Fields the tool deliberately doesn't expose stay None so the
        // service-side `if let Some(...)` guards leave them untouched.
        assert!(dto.birthdate.is_none());
        assert!(dto.dietary_goals.is_none());
        assert!(dto.is_active.is_none());
        // Writable fields forward verbatim.
        assert_eq!(dto.notes.as_deref(), Some("needs smaller portion"));
        assert_eq!(dto.favorites.unwrap(), vec!["mac and cheese".to_string()]);
        assert!(dto.dislikes.is_none());
        assert!(dto.drink_preferences.is_none());
        assert!(dto.drink_dislikes.is_none());
    }

    #[test]
    fn update_person_input_to_dto_normalizes_empty_notes_to_none() {
        // Empty and whitespace-only `notes` inputs must collapse to None
        // at this boundary — otherwise the caller could persist
        // `Some("")` as a back-door clear (the family-overview renderer
        // collapses it to `_none_`), silently forking the codebase
        // invariant that `notes` has no clear-to-NULL path. The four
        // list fields don't get the same treatment: `Some(vec![])` is
        // documented as a legitimate "replace with empty" write.
        for raw in ["", " ", "  \t\n  "] {
            let dto = update_person_input_to_dto(UpdatePersonInput {
                name: "Alice".into(),
                notes: Some(raw.into()),
                dislikes: None,
                favorites: None,
                drink_preferences: None,
                drink_dislikes: None,
            });
            assert!(
                dto.notes.is_none(),
                "empty/whitespace notes ({raw:?}) must coerce to None, got {:?}",
                dto.notes
            );
        }
        // Non-empty notes (even when they contain internal whitespace)
        // forward verbatim so we don't accidentally strip meaningful
        // content.
        let dto = update_person_input_to_dto(UpdatePersonInput {
            name: "Alice".into(),
            notes: Some("  needs smaller portion  ".into()),
            dislikes: None,
            favorites: None,
            drink_preferences: None,
            drink_dislikes: None,
        });
        assert_eq!(dto.notes.as_deref(), Some("  needs smaller portion  "));
    }
}
