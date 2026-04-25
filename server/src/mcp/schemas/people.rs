//! People-related MCP output types, conversions, and the Markdown renderer
//! backing the `fewd://family/overview` resource.

use schemars::JsonSchema;
use serde::Serialize;

use crate::entities::person;

use super::common::parse_json;

#[derive(Debug, Serialize, JsonSchema)]
pub struct PersonWithPrefs {
    pub name: String,
    pub dietary_goals: Option<String>,
    pub dislikes: Vec<String>,
    pub favorites: Vec<String>,
    pub notes: Option<String>,
}

pub fn person_to_prefs(person: &person::Model) -> Result<PersonWithPrefs, String> {
    let dislikes: Vec<String> = parse_json(&person.dislikes, "person dislikes")?;
    let favorites: Vec<String> = parse_json(&person.favorites, "person favorites")?;
    Ok(PersonWithPrefs {
        name: person.name.clone(),
        dietary_goals: person.dietary_goals.clone(),
        dislikes,
        favorites,
        notes: person.notes.clone(),
    })
}

/// Render active family members as Markdown for the `fewd://family/overview`
/// resource (and its tool mirror `get_family_overview`). Keeps every person's
/// dietary goals, likes, dislikes, and notes in one place so AI clients that
/// auto-load resources have immediate context.
///
/// Every person's block always emits all four bullets in the same order so
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
            drink_preferences: None,
            drink_dislikes: None,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
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
    }

    #[test]
    fn family_overview_with_no_people_is_explicit() {
        let out = render_family_overview(&[]).unwrap();
        assert!(out.contains("No active family members"));
    }

    #[test]
    fn family_overview_always_renders_every_bullet_even_when_empty() {
        // Every person section must emit all four bullets in the same
        // order so an "empty field" can't be mistaken for "not rendered".
        let mut p = mk_person("Vivienne");
        p.dietary_goals = None;
        p.dislikes = "[]".into();
        p.favorites = "[]".into();
        p.notes = None;

        let out = render_family_overview(&[p]).unwrap();
        assert!(out.contains("## Vivienne"));
        assert!(out.contains("**Dietary goals**: _none_"));
        assert!(out.contains("**Dislikes**: _none_"));
        assert!(out.contains("**Favorites**: _none_"));
        assert!(out.contains("**Notes**: _none_"));
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
    }

    #[test]
    fn person_to_prefs_rejects_malformed_json() {
        let mut p = mk_person("Broken");
        p.dislikes = "not-json".into();
        let err = person_to_prefs(&p).unwrap_err();
        assert!(err.contains("person dislikes"));
    }
}
