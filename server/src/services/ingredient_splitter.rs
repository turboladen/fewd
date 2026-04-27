//! Re-exports the ingredient name/prep splitter from the migration crate plus
//! a small server-side wrapper that applies the splitter defensively at
//! ingest boundaries.
//!
//! The canonical splitter lives in the migration crate (alongside the
//! backfill that walks every existing row through it) so the migration and
//! the runtime ingest paths can never drift. Server-side callers import
//! through this module to keep the call site readable.

pub use migration::split_name_and_prep;

/// Defensive normalization for `(name, prep)` pairs arriving at an ingest
/// boundary (MCP write tools, URL importer post-process). Coerces empty /
/// whitespace `prep` to `None` and applies the splitter when `name` carries
/// a comma'd prep clause that the upstream caller didn't peel off.
///
/// Idempotent: pairs that are already normalized (no comma in `name`, or
/// `prep` already populated with non-empty content) pass through unchanged.
pub fn normalize(name: String, prep: Option<String>) -> (String, Option<String>) {
    let prep = prep.filter(|s| !s.trim().is_empty());
    if prep.is_none() && name.contains(',') {
        return split_name_and_prep(&name);
    }
    (name, prep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_unsplit_name_when_prep_absent() {
        let (n, p) = normalize("garlic, minced".to_string(), None);
        assert_eq!(n, "garlic");
        assert_eq!(p.as_deref(), Some("minced"));
    }

    #[test]
    fn splits_when_prep_is_empty_string() {
        // LLM emitting `""` instead of `null` for an unset optional field.
        let (n, p) = normalize("garlic, minced".to_string(), Some(String::new()));
        assert_eq!(n, "garlic");
        assert_eq!(p.as_deref(), Some("minced"));
    }

    #[test]
    fn splits_when_prep_is_whitespace_only() {
        let (n, p) = normalize("garlic, minced".to_string(), Some("   ".to_string()));
        assert_eq!(n, "garlic");
        assert_eq!(p.as_deref(), Some("minced"));
    }

    #[test]
    fn passes_through_already_split_pair() {
        let (n, p) = normalize("garlic".to_string(), Some("minced".to_string()));
        assert_eq!(n, "garlic");
        assert_eq!(p.as_deref(), Some("minced"));
    }

    #[test]
    fn passes_through_no_comma_no_prep() {
        let (n, p) = normalize("olive oil".to_string(), None);
        assert_eq!(n, "olive oil");
        assert_eq!(p, None);
    }

    #[test]
    fn does_not_clobber_caller_prep_when_name_has_comma() {
        // Caller explicitly set prep — even if name has a comma, trust the
        // caller. We only split when prep is genuinely absent.
        let (n, p) = normalize("garlic, minced".to_string(), Some("smashed".to_string()));
        assert_eq!(n, "garlic, minced");
        assert_eq!(p.as_deref(), Some("smashed"));
    }
}
