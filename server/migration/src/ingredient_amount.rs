//! Amount parsing + unit recognition for ingredient lines.
//!
//! Canonical implementation lives here so both the runtime markdown parser
//! (`recipe_parser::parse_ingredient_line` via the server-side re-export)
//! and the backfill migrations that repair misbucketed rows read amounts and
//! units with the same code. A migration that has already run keeps the rows
//! it wrote, so a change here reaches only rows written or repaired later.
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

/// The dash characters that join the two ends of a range: ASCII `-`, en-dash
/// `–` and em-dash `—`.
pub const RANGE_DASHES: [char; 3] = ['-', '–', '—'];

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
    let dash_idx = s.find(RANGE_DASHES)?;
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

/// Canonical names of the weight and volume units, as [`normalize_unit`]
/// returns them.
pub const MEASURED_UNITS: &[&str] = &[
    "g", "kg", "mg", "oz", "lb", "ml", "dl", "cl", "l", "cup", "tbsp", "tsp", "pint", "quart",
    "gallon", "fl oz",
];

/// Canonical names of the discrete cooking units (cloves, cans, pinches),
/// as [`normalize_unit`] returns them.
pub const DISCRETE_UNITS: &[&str] = &[
    "whole",
    "piece",
    "clove",
    "slice",
    "can",
    "sheet",
    "head",
    "bunch",
    "sprig",
    "bag",
    "bottle",
    "package",
    "block",
    "stalk",
    "strip",
    "pinch",
    "dash",
    "splash",
    "drop",
    "leaf",
    "ball",
    "sleeve",
    "jar",
    "container",
    "stick",
    "ear",
    "shake",
];

/// Normalizes a unit string to one canonical name: trims, lowercases, reduces
/// a plural to its singular and maps aliases, so `"Tablespoons"`, `"tbs"` and
/// `"Tbsp."` all give `"tbsp"`. `"t"` gives `"tsp"`, and `"T"` or `"Ts"` gives
/// `"tbsp"`.
///
/// An abbreviation may end in one period; a full word with a period
/// (`"cups."`) comes back lowercase and otherwise unchanged. `"fl"` or
/// `"fluid"` before an ounce spelling gives `"fl oz"`. A word with no alias
/// comes back singular and lowercase, whether or not it is a unit. The result
/// normalizes to itself.
pub fn normalize_unit(unit: &str) -> String {
    let trimmed = unit.trim();
    // Only the capitalized forms are case-sensitive. All-caps "TS" stays the
    // teaspoon alias, because a recipe that writes every unit in capitals
    // cannot be told apart from one that uses capital T for tablespoon.
    match strip_period(trimmed).0 {
        "t" => return "tsp".to_string(),
        "T" | "Ts" => return "tbsp".to_string(),
        _ => {}
    }
    let lower = trimmed.to_lowercase();
    if is_fluid_ounce(&lower) {
        return "fl oz".to_string();
    }
    let (body, dotted) = strip_period(&lower);
    if !dotted && body.split_whitespace().eq(["to", "taste"]) {
        return "to taste".to_string();
    }
    let singular = singular_unit(body);
    if let Some(canonical) = abbreviation(singular) {
        return canonical.to_string();
    }
    // A period or a space left at the end of the singular ("oz.s", "cup s")
    // is not a spelling of any unit, and resolving it would make a second
    // call give a different result.
    if dotted || singular.ends_with(|c: char| c == '.' || c.is_whitespace()) {
        return lower;
    }
    full_word_alias(singular).unwrap_or(singular).to_string()
}

fn strip_period(s: &str) -> (&str, bool) {
    match s.strip_suffix('.') {
        Some(body) => (body, true),
        None => (s, false),
    }
}

// This matches "fl" or "fluid" followed by an ounce spelling ("fl oz",
// "fl. oz.", "fluid ounces"), and the packaging spelling "fl.oz" with no space.
// The ounce word follows the same period rule as a single unit.
fn is_fluid_ounce(lower: &str) -> bool {
    let mut words = lower.split_whitespace();
    let (first, ounce) = match (words.next(), words.next(), words.next()) {
        (Some(first), Some(ounce), None) => (first, ounce),
        (Some(word), None, None) => match word.strip_prefix("fl.") {
            Some(ounce) if !ounce.is_empty() => ("fl.", ounce),
            _ => return false,
        },
        _ => return false,
    };
    if !matches!(first, "fl" | "fl." | "fluid") {
        return false;
    }
    let (body, dotted) = strip_period(ounce);
    match singular_unit(body) {
        "oz" => true,
        "ounce" => !dotted,
        _ => false,
    }
}

// These short spellings may carry one trailing period ("oz.", "lbs.").
fn abbreviation(singular: &str) -> Option<&'static str> {
    Some(match singular {
        "g" => "g",
        "kg" => "kg",
        "mg" => "mg",
        "oz" => "oz",
        "lb" => "lb",
        "ml" => "ml",
        // dl and cl are units of their own rather than aliases of ml, because
        // the shopping list shows a sum in the unit this function returns.
        "dl" => "dl",
        "cl" => "cl",
        "l" => "l",
        "c" => "cup",
        "tbsp" | "tbs" | "tb" | "tbl" => "tbsp",
        "tsp" | "ts" => "tsp",
        "pt" => "pint",
        "qt" => "quart",
        "gal" => "gallon",
        "pkg" => "package",
        _ => return None,
    })
}

// Full words never take a period, because "cups." is a typo at the end of a
// sentence, so it stays unknown rather than becoming a unit.
fn full_word_alias(singular: &str) -> Option<&'static str> {
    Some(match singular {
        "gram" => "g",
        "kilogram" | "kilo" => "kg",
        "milligram" => "mg",
        "ounce" => "oz",
        "pound" => "lb",
        "milliliter" | "millilitre" => "ml",
        "deciliter" | "decilitre" => "dl",
        "centiliter" | "centilitre" => "cl",
        "liter" | "litre" => "l",
        "tablespoon" => "tbsp",
        "teaspoon" => "tsp",
        _ => return None,
    })
}

// Plurals that add `es` after `ch`, `sh`, `ss` or `x` lose both letters,
// "leaves" gives "leaf", a word ending in `ss` keeps it ("glass"), and any
// other trailing `s` is dropped when the word is longer than two letters, so
// "ts" stays the teaspoon alias. The `es` rule covers only sibilant endings,
// because the units ending in a plain `e` (slice, piece, shake) pluralize
// with a bare `s`. There is no `zes` or `ies` rule, because no unit needs one
// and a `zes` rule would cut "sizes" short.
fn singular_unit(s: &str) -> &str {
    if s == "leaves" {
        "leaf"
    } else if s.ends_with("ss") {
        s
    } else if ["ches", "shes", "sses", "xes"]
        .iter()
        .any(|ending| s.ends_with(ending))
    {
        &s[..s.len() - 2]
    } else if s.ends_with('s') && s.len() > 2 {
        &s[..s.len() - 1]
    } else {
        s
    }
}

/// Returns `true` if `token` looks like a recipe unit — weight, volume, or
/// a recognized discrete cooking unit. It tells a unit (`"6 cloves garlic"`)
/// from the first word of a compound name (`"1 zucchini, sliced"`).
///
/// A single trailing `,` is ignored. The rest passes through
/// [`normalize_unit`], so a period on an abbreviation (`"oz."`) and the
/// case of `"t"` and `"T"` count, and it is a unit when the result is in
/// [`MEASURED_UNITS`] or [`DISCRETE_UNITS`].
pub fn is_known_unit(token: &str) -> bool {
    let unit = normalize_unit(token.strip_suffix(',').unwrap_or(token));
    MEASURED_UNITS.contains(&unit.as_str()) || DISCRETE_UNITS.contains(&unit.as_str())
}

/// Splits a known unit off the front of `s`, the text that follows an
/// ingredient's amount, and returns the unit and the rest.
///
/// The first two words are tried as one unit (`"fl oz milk"`), then the first
/// word alone (`"cups flour"`). Words are separated by any run of whitespace,
/// and a two-word unit comes back joined by one space. A trailing `,` on the
/// unit is dropped (`"cloves, minced"` gives `"cloves"`); otherwise the unit
/// is returned as written, case and period included, for [`normalize_unit`]
/// to read later. The rest has its leading whitespace trimmed. Returns
/// `None` when no unit matches or nothing follows the unit.
pub fn split_leading_unit(s: &str) -> Option<(String, &str)> {
    let (first, after_first) = split_first_word(s)?;
    if let Some((second, rest)) = split_first_word(after_first) {
        let unit = format!("{first} {}", second.strip_suffix(',').unwrap_or(second));
        if !rest.is_empty() && is_known_unit(&unit) {
            return Some((unit, rest));
        }
    }
    let unit = first.strip_suffix(',').unwrap_or(first);
    if !after_first.is_empty() && is_known_unit(unit) {
        return Some((unit.to_string(), after_first));
    }
    None
}

/// Splits the first word off `s` at a run of whitespace and returns it with
/// the rest, whose leading whitespace is trimmed. Returns `None` when `s` is
/// blank.
pub fn split_first_word(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    Some(match s.split_once(char::is_whitespace) {
        Some((word, rest)) => (word, rest.trim_start()),
        None => (s, ""),
    })
}

/// Lookup table for Unicode vulgar fractions in the U+2150..U+215E block
/// plus the legacy Latin-1 code points (¼ ½ ¾). This is the whole set of
/// fractions the parser reads as a quantity.
pub fn unicode_fraction_value(c: char) -> Option<f64> {
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
    fn is_known_unit_accepts_weight_volume_discrete() {
        // Weight
        for unit in [
            "g", "gram", "grams", "kg", "lb", "lbs", "oz", "ounce", "ounces",
        ] {
            assert!(is_known_unit(unit), "expected {unit:?} to be a known unit");
        }
        // Volume
        for unit in [
            "ml",
            "l",
            "cup",
            "cups",
            "tbsp",
            "tablespoon",
            "tablespoons",
            "tsp",
            "teaspoon",
            "fl oz",
            "pint",
            "quart",
            "gallon",
        ] {
            assert!(is_known_unit(unit), "expected {unit:?} to be a known unit");
        }
        // Discrete cooking units
        for unit in [
            "whole",
            "piece",
            "pieces",
            "clove",
            "cloves",
            "slice",
            "slices",
            "can",
            "cans",
            "sheet",
            "head",
            "bunch",
            "sprig",
            "bag",
            "bottle",
            "package",
            "pkg",
            "block",
            "stalk",
            "stalks",
            "strip",
            "strips",
            "pinch",
            "dash",
            "splash",
            "drop",
            "leaf",
            "leaves",
            "ball",
            "balls",
            "sleeve",
            "jar",
            "container",
            "stick",
            "sticks",
            "ear",
            "shake",
        ] {
            assert!(is_known_unit(unit), "expected {unit:?} to be a known unit");
        }
    }

    #[test]
    fn is_known_unit_strips_trailing_comma() {
        // A word split leaves the comma on the unit token ("cloves," in
        // "6 cloves, minced garlic"), and the predicate must accept it.
        assert!(is_known_unit("cloves,"));
        assert!(is_known_unit("stalks,"));
    }

    #[test]
    fn is_known_unit_rejects_non_units() {
        // Real prod misparses — these tokens were stored as units but
        // aren't units.
        assert!(!is_known_unit("zucchini"));
        assert!(!is_known_unit("zucchini,"));
        assert!(!is_known_unit("scallions"));
        assert!(!is_known_unit("scallions,"));
        assert!(!is_known_unit("cucumber"));
        assert!(!is_known_unit("celery"));
        assert!(!is_known_unit("garlic"));
        assert!(!is_known_unit("bay"));
        // Size modifiers aren't units (treat them as part of the name).
        assert!(!is_known_unit("medium"));
        assert!(!is_known_unit("large"));
        assert!(!is_known_unit("small"));
        // Color modifiers aren't units.
        assert!(!is_known_unit("red"));
        assert!(!is_known_unit("green"));
        // Empty / garbage.
        assert!(!is_known_unit(""));
        assert!(!is_known_unit(","));
    }

    #[test]
    fn is_known_unit_accepts_es_plurals() {
        for unit in [
            "bunches", "pinches", "dashes", "splashes", "Bunches", "bunches,",
        ] {
            assert!(is_known_unit(unit), "expected {unit:?} to be a known unit");
        }
    }

    #[test]
    fn is_known_unit_rejects_words_the_plural_rule_must_not_invent() {
        for token in ["glass", "glasses", "boxes", "s", "ss", "to taste"] {
            assert!(!is_known_unit(token), "{token:?} is not a unit");
        }
    }

    #[test]
    fn normalize_unit_reduces_plurals_and_aliases() {
        for (input, expected) in [
            ("bunches", "bunch"),
            ("pinches", "pinch"),
            ("dashes", "dash"),
            ("splashes", "splash"),
            ("glasses", "glass"),
            ("glass", "glass"),
            ("boxes", "box"),
            ("leaves", "leaf"),
            ("slices", "slice"),
            ("shakes", "shake"),
            ("sizes", "size"),
            ("ts", "tsp"),
            ("tbs", "tbsp"),
            ("tb", "tbsp"),
            ("lbs", "lb"),
            ("cups", "cup"),
            ("Tbsp", "tbsp"),
            ("GRAMS", "g"),
            ("fl oz", "fl oz"),
            ("Fluid Ounces", "fl oz"),
            ("to taste", "to taste"),
            ("whole", "whole"),
            ("  Cloves ", "clove"),
        ] {
            assert_eq!(normalize_unit(input), expected, "normalize_unit({input:?})");
        }
    }

    #[test]
    fn unit_lists_hold_canonical_names() {
        for unit in MEASURED_UNITS.iter().chain(DISCRETE_UNITS) {
            assert_eq!(normalize_unit(unit), *unit, "{unit:?} must be canonical");
            assert!(is_known_unit(unit), "{unit:?} must be a known unit");
        }
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

    #[test]
    fn normalize_unit_reads_dotted_abbreviations() {
        for (input, expected) in [
            ("tsp.", "tsp"),
            ("Tbsp.", "tbsp"),
            ("oz.", "oz"),
            ("lb.", "lb"),
            ("lbs.", "lb"),
            ("tbs.", "tbsp"),
            ("tbl.", "tbsp"),
            ("g.", "g"),
            ("c.", "cup"),
            ("fl oz.", "fl oz"),
            ("T.", "tbsp"),
            ("t.", "tsp"),
        ] {
            assert_eq!(normalize_unit(input), expected, "normalize_unit({input:?})");
        }
    }

    #[test]
    fn normalize_unit_reads_t_and_capital_t_as_different_units() {
        for (input, expected) in [("t", "tsp"), ("T", "tbsp"), (" T ", "tbsp"), ("TS", "tsp")] {
            assert_eq!(normalize_unit(input), expected, "normalize_unit({input:?})");
        }
    }

    #[test]
    fn normalize_unit_reads_short_metric_and_cup_spellings() {
        for (input, expected) in [
            ("c", "cup"),
            ("C", "cup"),
            ("dl", "dl"),
            ("deciliters", "dl"),
            ("cl", "cl"),
            ("centilitre", "cl"),
            ("kilo", "kg"),
            ("kilos", "kg"),
            ("tbl", "tbsp"),
        ] {
            assert_eq!(normalize_unit(input), expected, "normalize_unit({input:?})");
        }
    }

    #[test]
    fn normalize_unit_reads_fluid_oz() {
        for input in [
            "fluid oz",
            "Fluid Ozs",
            "fluid oz.",
            "fl. oz.",
            "fl ounces",
            "fl  oz",
            "fl ozs",
            "fl. ounce",
            "fl.oz",
            "fl.oz.",
        ] {
            assert_eq!(normalize_unit(input), "fl oz", "normalize_unit({input:?})");
        }
    }

    #[test]
    fn is_known_unit_rejects_words_the_period_rule_must_not_invent() {
        for token in [
            "cups.", "glass.", "st.", "no.", "in.", "ea.", "oz..", ".", "..", "s.", "T-bone",
        ] {
            assert!(!is_known_unit(token), "{token:?} is not a unit");
        }
    }

    #[test]
    fn is_known_unit_strips_a_comma_after_a_period() {
        assert!(is_known_unit("oz.,"));
        assert!(is_known_unit("tsp.,"));
    }

    #[test]
    fn normalize_unit_is_idempotent() {
        let inputs = [
            "tsp.",
            "Tbsp.",
            "oz.",
            "lbs.",
            "t",
            "T",
            "t.",
            "T.",
            "c",
            "C",
            "dl",
            "cl",
            "kilo",
            "kilos",
            "tbl",
            "fluid oz",
            "Fluid Ozs",
            "cups.",
            "glass.",
            "oz..",
            ".",
            "bunches",
            "leaves",
            "Tablespoons",
            "to taste",
            "",
            "oz.s",
            "lb.s",
            "fl oz.s",
            "fl oz..",
            "fluid ounces.",
            "fl.oz.",
            "Ts",
            "Ts.",
            "TS",
            "pkg",
            "cup s",
            "ttt s",
            "tts S",
            "to  taste",
        ];
        for input in inputs.iter().chain(MEASURED_UNITS).chain(DISCRETE_UNITS) {
            let once = normalize_unit(input);
            assert_eq!(normalize_unit(&once), once, "normalize_unit({input:?})");
        }
    }

    #[test]
    fn split_leading_unit_takes_one_or_two_words() {
        for (input, unit, rest) in [
            ("cups flour", "cups", "flour"),
            ("cloves, minced garlic", "cloves", "minced garlic"),
            ("fl oz milk", "fl oz", "milk"),
            ("fluid oz milk", "fluid oz", "milk"),
            ("fl. oz. milk", "fl. oz.", "milk"),
            ("fl oz, milk", "fl oz", "milk"),
            ("Tbsp. butter", "Tbsp.", "butter"),
            ("fl  oz   milk", "fl oz", "milk"),
            ("T butter, softened", "T", "butter, softened"),
        ] {
            assert_eq!(
                split_leading_unit(input),
                Some((unit.to_string(), rest)),
                "split_leading_unit({input:?})"
            );
        }
    }

    #[test]
    fn split_leading_unit_needs_a_unit_and_a_rest() {
        for input in [
            "fl oz",
            "cups",
            "zucchini, sliced",
            "",
            "   ",
            "T-bone steak",
        ] {
            assert_eq!(
                split_leading_unit(input),
                None,
                "split_leading_unit({input:?})"
            );
        }
    }

    #[test]
    fn fluid_ounce_follows_the_period_rules() {
        for input in [
            "fl oz..",
            "fluid ounces.",
            "fl ounces.",
            "fl.",
            "fl. cups",
            "fluid",
        ] {
            assert_ne!(normalize_unit(input), "fl oz", "normalize_unit({input:?})");
        }
    }

    #[test]
    fn capitalized_t_plurals_are_tablespoons_and_all_caps_stays_teaspoon() {
        for (input, expected) in [
            ("Ts", "tbsp"),
            ("Ts.", "tbsp"),
            ("ts", "tsp"),
            ("TS", "tsp"),
        ] {
            assert_eq!(normalize_unit(input), expected, "normalize_unit({input:?})");
        }
    }

    #[test]
    fn pkg_is_package() {
        assert_eq!(normalize_unit("pkg"), "package");
        assert_eq!(normalize_unit("pkgs."), "package");
        assert!(is_known_unit("pkg"));
    }

    #[test]
    fn a_period_before_a_plural_s_stays_unknown() {
        for input in ["oz.s", "lb.s", "fl oz.s"] {
            assert!(!is_known_unit(input), "{input:?} is not a unit");
        }
    }

    #[test]
    fn a_lone_s_after_a_space_stays_unknown() {
        for input in ["cup s", "oz s"] {
            assert_eq!(normalize_unit(input), input, "normalize_unit({input:?})");
            assert!(!is_known_unit(input), "{input:?} is not a unit");
        }
    }

    #[test]
    fn to_taste_with_extra_spaces_is_to_taste() {
        for input in ["to  taste", "To\tTaste", " to taste "] {
            assert_eq!(
                normalize_unit(input),
                "to taste",
                "normalize_unit({input:?})"
            );
        }
        assert_eq!(normalize_unit("to taste."), "to taste.");
    }
}
