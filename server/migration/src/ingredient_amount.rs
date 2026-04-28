//! Amount parsing + unit recognition for ingredient lines.
//!
//! Canonical implementation lives here so both the runtime markdown parser
//! (`recipe_parser::parse_ingredient_line` via the server-side re-export)
//! and the backfill migration that repairs misbucketed prod rows share one
//! source of truth — they cannot drift on what counts as a parseable amount
//! or a known unit.
//!
//! Recognized amount forms:
//! - Plain integers and decimals (`"2"`, `"1.5"`).
//! - ASCII fractions `a/b` where `1 ≤ a ≤ 9` and `2 ≤ b ≤ 16` (`"1/2"`,
//!   `"3/4"`). The bounds reject labels like `"80/20"` that are not real
//!   cooking fractions.
//! - Ranges with any dash variant — ASCII `-`, en-dash `–`, em-dash `—`
//!   (`"1-2"`, `"12–15"`, `"3—4"`).
//! - Standalone Unicode vulgar fractions (`"¼"`, `"⅔"`).
//! - Mixed Unicode form `<digits><vulgar fraction>` (`"1½"`, `"2¼"`).

use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AmountKind {
    Single(f64),
    Range { min: f64, max: f64 },
}

/// Parse an amount token from a recipe line. Returns `None` on garbage,
/// labels that look like fractions but aren't (`"80/20"`), or empty input.
pub fn try_parse_amount(s: &str) -> Option<AmountKind> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Range: split on first dash variant. Each side is parsed via the
    // simple-amount path (no nested ranges) so a malformed `"1-1-2"` falls
    // through to None rather than silently picking one of the dashes.
    if let Some((min_s, max_s)) = split_on_dash(s) {
        let min = parse_simple_amount(min_s)?;
        let max = parse_simple_amount(max_s)?;
        return Some(AmountKind::Range { min, max });
    }

    parse_simple_amount(s).map(AmountKind::Single)
}

/// JSON-shaped tagged amount, the form the migration writes back into the
/// recipes.ingredients column.
pub fn try_parse_amount_json(s: &str) -> Option<Value> {
    match try_parse_amount(s)? {
        AmountKind::Single(value) => Some(json!({ "type": "single", "value": value })),
        AmountKind::Range { min, max } => Some(json!({ "type": "range", "min": min, "max": max })),
    }
}

fn split_on_dash(s: &str) -> Option<(&str, &str)> {
    let dash_idx = s.find(['-', '–', '—'])?;
    // Splitting at the byte index of the dash; multi-byte dashes are handled
    // because find returns a byte index and we slice with the dash's char
    // length.
    let dash_char = s[dash_idx..].chars().next()?;
    let after_dash = dash_idx + dash_char.len_utf8();
    Some((&s[..dash_idx], &s[after_dash..]))
}

fn parse_simple_amount(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Plain integer or decimal.
    if let Ok(value) = s.parse::<f64>() {
        return Some(value);
    }

    // ASCII fraction with sanity bounds (cooking fractions are small).
    if let Some((num_s, den_s)) = s.split_once('/') {
        if let (Ok(num), Ok(den)) = (num_s.trim().parse::<u32>(), den_s.trim().parse::<u32>()) {
            if (1..=9).contains(&num) && (2..=16).contains(&den) {
                return Some(num as f64 / den as f64);
            }
        }
        return None;
    }

    // Standalone Unicode vulgar fraction.
    let mut chars = s.chars();
    let first = chars.next()?;
    if chars.next().is_none() {
        return unicode_fraction_value(first);
    }

    // Mixed form: leading digits + trailing single vulgar fraction char.
    let last = s.chars().next_back()?;
    if let Some(frac) = unicode_fraction_value(last) {
        let prefix_end = s.len() - last.len_utf8();
        let prefix = &s[..prefix_end];
        if let Ok(integer) = prefix.parse::<u32>() {
            return Some(integer as f64 + frac);
        }
    }

    None
}

/// Lookup table for Unicode vulgar fractions in the U+2150..U+215E block
/// plus the legacy Latin-1 code points (¼ ½ ¾).
fn unicode_fraction_value(c: char) -> Option<f64> {
    match c {
        '¼' => Some(0.25),
        '½' => Some(0.5),
        '¾' => Some(0.75),
        '⅓' => Some(1.0 / 3.0),
        '⅔' => Some(2.0 / 3.0),
        '⅕' => Some(0.2),
        '⅖' => Some(0.4),
        '⅗' => Some(0.6),
        '⅘' => Some(0.8),
        '⅙' => Some(1.0 / 6.0),
        '⅚' => Some(5.0 / 6.0),
        '⅛' => Some(0.125),
        '⅜' => Some(0.375),
        '⅝' => Some(0.625),
        '⅞' => Some(0.875),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn single(v: f64) -> AmountKind {
        AmountKind::Single(v)
    }

    fn range(min: f64, max: f64) -> AmountKind {
        AmountKind::Range { min, max }
    }

    #[test]
    fn ascii_integer_and_decimal() {
        assert_eq!(try_parse_amount("2"), Some(single(2.0)));
        assert_eq!(try_parse_amount("1.5"), Some(single(1.5)));
        assert_eq!(try_parse_amount("0.25"), Some(single(0.25)));
    }

    #[test]
    fn ascii_fraction_in_bounds() {
        assert_eq!(try_parse_amount("1/2"), Some(single(0.5)));
        assert_eq!(try_parse_amount("3/4"), Some(single(0.75)));
        assert_eq!(try_parse_amount("1/16"), Some(single(0.0625)));
        assert_eq!(try_parse_amount("9/16"), Some(single(0.5625)));
    }

    #[test]
    fn ascii_fraction_out_of_bounds_rejected() {
        // 80/20 is a label, not a fraction — numerator too big.
        assert_eq!(try_parse_amount("80/20"), None);
        assert_eq!(try_parse_amount("100/200"), None);
        assert_eq!(try_parse_amount("10/2"), None);
        // Zero denominator.
        assert_eq!(try_parse_amount("1/0"), None);
        // Zero numerator.
        assert_eq!(try_parse_amount("0/2"), None);
        // Non-numeric.
        assert_eq!(try_parse_amount("a/b"), None);
    }

    #[test]
    fn range_ascii_dash() {
        assert_eq!(try_parse_amount("1-2"), Some(range(1.0, 2.0)));
        assert_eq!(try_parse_amount("2-3"), Some(range(2.0, 3.0)));
    }

    #[test]
    fn range_en_dash() {
        assert_eq!(try_parse_amount("12–15"), Some(range(12.0, 15.0)));
        assert_eq!(try_parse_amount("3–4"), Some(range(3.0, 4.0)));
    }

    #[test]
    fn range_em_dash() {
        assert_eq!(try_parse_amount("2—3"), Some(range(2.0, 3.0)));
    }

    #[test]
    fn standalone_unicode_fractions() {
        assert_eq!(try_parse_amount("¼"), Some(single(0.25)));
        assert_eq!(try_parse_amount("½"), Some(single(0.5)));
        assert_eq!(try_parse_amount("¾"), Some(single(0.75)));
        assert_eq!(try_parse_amount("⅛"), Some(single(0.125)));
        let third = try_parse_amount("⅓").unwrap();
        match third {
            AmountKind::Single(v) => assert!((v - 1.0 / 3.0).abs() < 1e-9),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn mixed_unicode_fractions() {
        assert_eq!(try_parse_amount("1½"), Some(single(1.5)));
        assert_eq!(try_parse_amount("2¼"), Some(single(2.25)));
        let one_third = try_parse_amount("1⅓").unwrap();
        match one_third {
            AmountKind::Single(v) => assert!((v - (1.0 + 1.0 / 3.0)).abs() < 1e-9),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(try_parse_amount("flour"), None);
        assert_eq!(try_parse_amount(""), None);
        assert_eq!(try_parse_amount("1x"), None);
        // Range with nested range or invalid side rejects.
        assert_eq!(try_parse_amount("1-1-2"), None);
        assert_eq!(try_parse_amount("a-b"), None);
    }

    #[test]
    fn json_form_round_trips() {
        assert_eq!(
            try_parse_amount_json("2.5"),
            Some(json!({ "type": "single", "value": 2.5 }))
        );
        assert_eq!(
            try_parse_amount_json("12–15"),
            Some(json!({ "type": "range", "min": 12.0, "max": 15.0 }))
        );
        assert_eq!(try_parse_amount_json("80/20"), None);
    }
}
