//! Peels a mid-string size-info parenthetical out of an ingredient string,
//! returning the cleaned line plus the parenthetical content as `notes`.
//!
//! Strategy: scan for the *first* `(...)` and peel it only when **both**
//! conditions hold:
//!
//! 1. The suffix immediately after the closing `)` starts with whitespace +
//!    word chars (NOT a comma). A comma suffix is the fewd-xez precedent
//!    (`pear (or Fuji apple), grated`) where the parens carry an alternative
//!    noun, not size info — there the downstream splitter must see the
//!    string intact so it can find the top-level comma.
//! 2. The parenthetical content contains at least one token recognized by
//!    [`crate::ingredient_amount::is_known_unit`] (oz, lb, ml, g, …). This
//!    disambiguates *size info* (`(28 oz each)`) from *alternative nouns*
//!    (`(or Fuji apple)`).
//!
//! When peeled, the surrounding whitespace is collapsed to a single space.
//! When neither condition holds, the input is returned unchanged.
//!
//! Shared between the runtime parser (`recipe_parser::parse_ingredient_line`)
//! and the backfill migration that repairs dietpi rows already corrupted
//! by the pre-fix parser. See fewd-i47 for the bug history.

use crate::ingredient_amount::is_known_unit;

/// Peel a mid-string size-info parenthetical from `s`, returning
/// `(cleaned, Some(notes))` if the heuristic matches, or `(s.to_string(), None)`
/// otherwise.
pub fn peel_size_paren(s: &str) -> (String, Option<String>) {
    let Some(open) = s.find('(') else {
        return (s.to_string(), None);
    };
    let Some(close_rel) = s[open + 1..].find(')') else {
        return (s.to_string(), None);
    };
    let close = open + 1 + close_rel;

    let inner = s[open + 1..close].trim();
    if inner.is_empty() {
        return (s.to_string(), None);
    }

    let suffix = &s[close + 1..];
    if !suffix_starts_with_word_chars(suffix) {
        return (s.to_string(), None);
    }

    if !contains_unit_token(inner) {
        return (s.to_string(), None);
    }

    let prefix = s[..open].trim_end();
    let suffix = suffix.trim_start();
    let cleaned = if prefix.is_empty() {
        suffix.to_string()
    } else if suffix.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix} {suffix}")
    };

    if cleaned.is_empty() {
        return (s.to_string(), None);
    }

    (cleaned, Some(inner.to_string()))
}

/// True if `suffix` begins with whitespace followed by an alphanumeric
/// character. A leading comma (or `,` after whitespace) returns false —
/// that case must defer to `split_name_and_prep` so fewd-xez behavior is
/// preserved.
fn suffix_starts_with_word_chars(suffix: &str) -> bool {
    let mut chars = suffix.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_whitespace() {
        return false;
    }
    let Some(second) = chars.find(|c| !c.is_whitespace()) else {
        return false;
    };
    second.is_alphanumeric()
}

/// True if any whitespace-delimited token inside the parenthetical is a
/// known unit. ASCII punctuation is stripped off each candidate so `"oz,"`
/// and `"oz."` still match while `"28oz"` (no separator) does not.
fn contains_unit_token(inner: &str) -> bool {
    inner.split_whitespace().any(|tok| {
        let stripped = tok.trim_matches(|c: char| !c.is_alphanumeric());
        !stripped.is_empty() && is_known_unit(stripped)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peel(s: &str) -> (String, Option<String>) {
        peel_size_paren(s)
    }

    #[test]
    fn mid_string_size_parens_with_word_suffix_are_peeled() {
        // The fewd-i47 hero case: parens carry per-can size info, noun
        // trails, no comma between `)` and the noun.
        assert_eq!(
            peel("2 cans (28 oz each) crushed San Marzano tomatoes"),
            (
                "2 cans crushed San Marzano tomatoes".to_string(),
                Some("28 oz each".to_string())
            )
        );
    }

    #[test]
    fn mid_string_parens_with_comma_suffix_are_left_alone() {
        // The fewd-xez precedent: parens are an alternative noun, not size
        // info. Even though a unit token might appear, the comma suffix
        // signals deferral to split_name_and_prep.
        assert_eq!(
            peel("pear (or Fuji apple), grated"),
            ("pear (or Fuji apple), grated".to_string(), None)
        );
        // Even when the parens DO carry a unit token, comma-suffix wins —
        // we don't want to fight the splitter for prep clauses.
        assert_eq!(
            peel("chicken (about 1 lb), boneless"),
            ("chicken (about 1 lb), boneless".to_string(), None)
        );
    }

    #[test]
    fn mid_string_parens_without_unit_token_are_left_alone() {
        // The "alternative noun" case without a comma suffix. Without a
        // unit token to anchor the heuristic we leave the string alone so
        // the parser preserves the parens in `name`.
        assert_eq!(
            peel("Asian pear (or Fuji apple) grated"),
            ("Asian pear (or Fuji apple) grated".to_string(), None)
        );
    }

    #[test]
    fn trailing_parens_are_left_alone() {
        // `extract_notes` owns the trailing-paren case. We must not touch
        // it here — the suffix after `)` is empty, not "whitespace + word".
        assert_eq!(
            peel("orange juice (fresh is best)"),
            ("orange juice (fresh is best)".to_string(), None)
        );
    }

    #[test]
    fn no_parens_returns_input_unchanged() {
        assert_eq!(peel("garlic"), ("garlic".to_string(), None));
        assert_eq!(peel("2 cups flour"), ("2 cups flour".to_string(), None));
    }

    #[test]
    fn empty_parens_are_left_alone() {
        assert_eq!(peel("foo () bar"), ("foo () bar".to_string(), None));
    }

    #[test]
    fn mismatched_parens_are_left_alone() {
        // Open paren but no close.
        assert_eq!(peel("foo (bar baz"), ("foo (bar baz".to_string(), None));
    }

    #[test]
    fn parens_with_lb_unit_token_are_peeled() {
        // Confirms the unit detection works for weight units, not just oz.
        assert_eq!(
            peel("3 chicken breasts (about 1 lb total) boneless"),
            (
                "3 chicken breasts boneless".to_string(),
                Some("about 1 lb total".to_string())
            )
        );
    }

    #[test]
    fn parens_with_g_unit_token_are_peeled() {
        // Single-letter unit token mid-parenthetical.
        assert_eq!(
            peel("1 onion (200 g) chopped"),
            ("1 onion chopped".to_string(), Some("200 g".to_string()))
        );
    }

    #[test]
    fn fused_unit_token_without_separator_is_left_alone() {
        // The doc on `contains_unit_token` says fused tokens like "28oz"
        // (no separator) won't match. Pin that boundary in a test so a
        // future tokenizer rewrite doesn't silently change it.
        assert_eq!(
            peel("2 cans (28oz each) crushed tomatoes"),
            ("2 cans (28oz each) crushed tomatoes".to_string(), None)
        );
    }

    #[test]
    fn idempotent_on_already_peeled_string() {
        let (peeled, _) = peel("2 cans (28 oz each) crushed San Marzano tomatoes");
        let (peeled2, notes2) = peel(&peeled);
        assert_eq!(peeled, peeled2);
        assert_eq!(notes2, None);
    }

    #[test]
    fn whitespace_collapsed_around_peel_site() {
        // Make sure we don't end up with "  cans  crushed".
        let (cleaned, notes) = peel("2 cans (28 oz each) crushed tomatoes");
        assert!(!cleaned.contains("  "), "double space in {cleaned:?}");
        assert_eq!(notes.as_deref(), Some("28 oz each"));
    }
}
