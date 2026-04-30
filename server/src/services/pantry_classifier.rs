//! Pantry-staple classifier for the shopping list.
//!
//! Given an aggregated shopping-list line `(name, unit, total_amount)`,
//! decides whether it represents a household staple the user almost
//! certainly already has on hand (and therefore should *verify*) versus a
//! genuine shopping need.
//!
//! Two rules, in order:
//!
//! 1. **Small-measurement units → staple.** `tsp`, `tbsp`, `pinch`, `dash`,
//!    `splash`, `drop`, `shake` almost universally indicate a spice or
//!    seasoning bought in a jar and used sparingly. The unit alone is
//!    diagnostic.
//!
//! 2. **Name allowlist.** For larger/discrete units (`cup`, `oz`, `lb`,
//!    `whole`, etc.) the unit is not diagnostic, so we fall back to a
//!    curated list of names the household keeps stocked. Match is *exact*
//!    after normalization (lowercase + trim + collapse internal whitespace)
//!    to avoid the substring traps: an entry of `"pepper"` would otherwise
//!    misclassify "bell pepper", and `"salt"` would misclassify "salt cod".
//!
//! False negatives cost less than false positives in this classifier,
//! because consumers present the staples as *"verify these"*, not *"don't
//! buy these"*. With "is staple" as the positive class: a real staple
//! slipped into `items_to_buy` (false negative) is just noise on the
//! shopping list; a real shopping item miscategorized as a staple
//! (false positive) hides in the verify section and may not get bought.

use crate::dto::IngredientAmountDto;
use crate::services::unit_converter::normalize_unit;

/// Returns `true` if the aggregated shopping-list line represents a
/// pantry staple the user likely already has.
///
/// `total_amount` is unused today but plumbed through so a future
/// amount-aware rule (e.g. "1 cup butter is a primary ingredient, not a
/// staple") can land without churning call sites. `None` means the
/// upstream aggregator couldn't compute a total — it's a real signal,
/// not a missing value, and a future amount-aware rule should fall
/// through to the name-only decision when it sees `None`.
pub fn is_pantry_staple(
    name: &str,
    unit: Option<&str>,
    _total_amount: Option<&IngredientAmountDto>,
) -> bool {
    if let Some(u) = unit {
        if is_small_measurement_unit(u) {
            return true;
        }
    }
    is_allowlisted_staple(name)
}

/// Units small enough that any ingredient measured in them is almost
/// certainly a seasoning or spice — diagnostic regardless of name.
fn is_small_measurement_unit(unit: &str) -> bool {
    matches!(
        normalize_unit(unit).as_str(),
        "tsp" | "tbsp" | "pinch" | "dash" | "splash" | "drop" | "shake"
    )
}

fn is_allowlisted_staple(name: &str) -> bool {
    let normalized = normalize_name(name);
    STAPLE_NAMES.iter().any(|s| *s == normalized)
}

/// Lowercase, trim, collapse internal whitespace runs. Mirrors the same
/// shape we use to compare allowlist entries.
fn normalize_name(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    lower.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Names of ingredients the household keeps on hand by default.
///
/// Match rules (enforced by `is_allowlisted_staple` + `normalize_name`):
/// - **Exact match only** after lowercase + trim + whitespace-collapse.
///   `"pepper"` will NOT match "bell pepper"; `"salt"` will NOT match
///   "salt cod". This is the whole point — substring matching gives wrong
///   answers on common cooking ingredients.
/// - **Common modifiers must be listed explicitly.** If you keep both
///   "kosher salt" and "table salt", list both. If "ground black pepper"
///   should match alongside "black pepper", list both.
/// - **Lowercase only.** Entries are matched after the input is lowercased,
///   so writing entries lowercase makes the data and the comparison match.
///
/// Expand as false negatives surface — recipes that call for something
/// that's clearly a staple but slipped through.
#[rustfmt::skip]
const STAPLE_NAMES: &[&str] = &[
    // salts
    "kosher salt",
    "salt",
    "sea salt",
    "table salt", 
    // peppers
    "black pepper",
    "ground black pepper",
    "white pepper",
    // oils
    "canola oil",
    "extra virgin olive oil",
    "olive oil",
    "vegetable oil",
    // fats
    "butter",
    "salted butter",
    // baking
    "flour",
    "all-purpose flour",
    "sugar",
    "granulated sugar",
    "brown sugar",
    "baking powder",
    "baking soda", 
    // acids
    "white vinegar",
    "apple cider vinegar",
    "balsamic vinegar",
    "red wine vinegar",
    "rice vinegar",
    // spices
    "allspice",
    "bay leaves",
    "cardamom",
    "cayenne",
    "chili powder",
    "cinnamon",
    "crushed red pepper",
    "cumin",
    "dried basil",
    "dried oregano",
    "garlic powder",
    "ground cinnamon",
    "ground cumin",
    "nutmeg",
    "onion powder",
    "oregano",
    "paprika",
    "red pepper flakes",
    "rosemary",
    "smoked paprika",
    "tarragon",
    "thyme",
    // sauces
    "fish sauce",
    "hot sauce",
    "soy sauce",
    // condiments
    "yellow mustard",
    "mustard",
    "mayonnaise",
    "ketchup",
    // flavoring
    "vanilla extract",
    "maple syrup",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn single(value: f64) -> IngredientAmountDto {
        IngredientAmountDto::Single { value }
    }

    // ── Rule 1: small-measurement units ─────────────────────────────

    #[test]
    fn tsp_is_staple_regardless_of_name() {
        // Picks a name we know is NOT in STAPLE_NAMES so a passing test
        // proves Rule 1 fired, not the name fallback.
        let amt = single(1.0);
        assert!(is_pantry_staple("chicken broth", Some("tsp"), Some(&amt)));
    }

    #[test]
    fn tbsp_is_staple_case_insensitive() {
        let amt = single(2.0);
        assert!(is_pantry_staple("anything", Some("Tbsp"), Some(&amt)));
        assert!(is_pantry_staple("anything", Some("TBSP"), Some(&amt)));
    }

    #[test]
    fn teaspoon_long_form_is_staple() {
        let amt = single(1.0);
        assert!(is_pantry_staple("anything", Some("teaspoon"), Some(&amt)));
        assert!(is_pantry_staple("anything", Some("tablespoon"), Some(&amt)));
    }

    #[test]
    fn pinch_dash_splash_drop_shake_are_staples() {
        let amt = single(1.0);
        for unit in ["pinch", "dash", "splash", "drop", "shake"] {
            assert!(
                is_pantry_staple("anything", Some(unit), Some(&amt)),
                "{unit} should classify as a staple",
            );
        }
    }

    // ── Rule 2: name allowlist ──────────────────────────────────────

    #[test]
    fn non_staple_names_are_not_classified() {
        let amt = single(2.0);
        for name in ["chicken breast", "carrot", "tomato", "ground beef"] {
            assert!(
                !is_pantry_staple(name, Some("lb"), Some(&amt)),
                "{name} should NOT classify as a staple",
            );
        }
    }

    #[test]
    fn allowlisted_names_are_staples_in_non_small_units() {
        // Each case uses a unit that does NOT trigger Rule 1, isolating
        // the name-allowlist path from the small-unit fallback.
        let amt = single(1.0);
        let cases = [
            ("kosher salt", "lb"),
            ("olive oil", "cup"),
            ("butter", "oz"),
            ("flour", "cup"),
            ("baking powder", "oz"),
            ("apple cider vinegar", "cup"),
            ("smoked paprika", "oz"),
            ("soy sauce", "cup"),
            ("vanilla extract", "oz"),
            ("maple syrup", "cup"),
        ];
        for (name, unit) in cases {
            assert!(
                is_pantry_staple(name, Some(unit), Some(&amt)),
                "{name} ({unit}) should classify as a staple via the allowlist",
            );
        }
    }

    #[test]
    fn substring_traps_must_not_match_allowlist() {
        // Each entry shares a substring with an allowlisted name but is a
        // distinct grocery item. Exact-match (not contains-match) on the
        // allowlist is what protects these from misclassification.
        let amt = single(1.0);
        let traps = [
            ("bell pepper", "whole"),    // shares "pepper"
            ("salt cod", "lb"),          // shares "salt"
            ("olives", "oz"),            // shares "olive oil"-ish
            ("garlic", "clove"),         // shares "garlic powder"
            ("onion", "whole"),          // shares "onion powder"
            ("cinnamon roll", "whole"),  // shares "cinnamon"
            ("sugar snap pea", "oz"),    // shares "sugar"
            ("flour tortilla", "whole"), // shares "flour"
            ("buttermilk", "cup"),       // shares "butter"
            ("mustard greens", "bunch"), // shares "mustard"
        ];
        for (name, unit) in traps {
            assert!(
                !is_pantry_staple(name, Some(unit), Some(&amt)),
                "{name} ({unit}) must NOT match the allowlist by substring",
            );
        }
    }

    #[test]
    fn allowlist_match_is_case_and_whitespace_insensitive() {
        let amt = single(0.5);
        for variant in ["Olive Oil", "OLIVE OIL", "  olive   oil  ", "olive\toil"] {
            assert!(
                is_pantry_staple(variant, Some("cup"), Some(&amt)),
                "variant {variant:?} should normalize to an allowlist hit",
            );
        }
    }

    // ── Edge: missing fields ────────────────────────────────────────

    #[test]
    fn missing_unit_falls_through_to_name_rule() {
        // When the aggregator can't agree on a unit across sources,
        // total_unit is None — classifier should give a name-only answer.
        let amt = single(2.0);
        assert!(!is_pantry_staple("chicken breast", None, Some(&amt)));
        assert!(is_pantry_staple("kosher salt", None, Some(&amt)));
    }

    #[test]
    fn missing_total_amount_does_not_panic() {
        // total_amount=None means the aggregator returned no usable total
        // (incompatible units across sources). Classifier still answers.
        assert!(!is_pantry_staple("chicken breast", Some("lb"), None));
        assert!(is_pantry_staple("kosher salt", Some("lb"), None));
        assert!(is_pantry_staple("anything", Some("tsp"), None));
    }

    // ── normalize_name + helpers ────────────────────────────────────

    #[test]
    fn name_normalization_is_lowercase_trim_collapse() {
        assert_eq!(normalize_name("  Olive   Oil "), "olive oil");
        assert_eq!(normalize_name("KOSHER SALT"), "kosher salt");
        assert_eq!(normalize_name("garlic\tpowder"), "garlic powder");
    }

    #[test]
    fn small_unit_detector_normalizes_plurals() {
        // `unit_converter::normalize_unit` strips a trailing 's' on simple
        // plurals; "tsps" → "tsp" → small unit. Note: irregular plurals
        // like "pinches" (which would need -es stripping) aren't handled —
        // recipe text near-universally writes "1 pinch" or "a pinch", so
        // the gap doesn't bite in practice.
        assert!(is_small_measurement_unit("tsps"));
        assert!(is_small_measurement_unit("tablespoons"));
        assert!(is_small_measurement_unit("drops"));
        assert!(is_small_measurement_unit("shakes"));
    }

    #[test]
    fn ordinary_units_are_not_small() {
        for unit in ["cup", "oz", "lb", "g", "ml", "whole", "clove"] {
            assert!(
                !is_small_measurement_unit(unit),
                "{unit} should not be a small measurement",
            );
        }
    }
}
