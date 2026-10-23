//! Cooking unit converter for shopping list aggregation.
//!
//! Converts between weight units (base: grams) and volume units (base: ml).
//! Discrete/uncategorized units (whole, piece, etc.) are not converted.

use serde::{Deserialize, Serialize};

/// Returns the unit category: "weight", "volume", or None for discrete/unknown units.
pub fn unit_category(unit: &str) -> Option<&'static str> {
    let normalized = normalize_unit(unit);
    if weight_to_grams(&normalized).is_some() {
        Some("weight")
    } else if volume_to_ml(&normalized).is_some() {
        Some("volume")
    } else {
        None
    }
}

/// Convert a value from the given unit to base units (grams for weight, ml for volume).
pub fn to_base(value: f64, unit: &str) -> Option<f64> {
    let normalized = normalize_unit(unit);
    weight_to_grams(&normalized)
        .or_else(|| volume_to_ml(&normalized))
        .map(|factor| value * factor)
}

/// Convert a value from base units back to a target unit.
pub fn from_base(base_value: f64, target_unit: &str) -> Option<f64> {
    let normalized = normalize_unit(target_unit);
    weight_to_grams(&normalized)
        .or_else(|| volume_to_ml(&normalized))
        .map(|factor| base_value / factor)
}

/// Pick the most readable display unit for a base value in a given category.
/// Returns (converted_value, unit_name).
pub fn best_display_unit(base_value: f64, category: &str) -> (f64, String) {
    match category {
        "weight" => best_weight_unit(base_value),
        "volume" => best_volume_unit(base_value),
        _ => (base_value, String::new()),
    }
}

/// Normalize a unit string: lowercase, trim, strip trailing 's' for plurals.
pub fn normalize_unit(unit: &str) -> String {
    let s = unit.trim().to_lowercase();
    // Handle multi-word units first
    match s.as_str() {
        "fl oz" | "fl. oz" | "fl. oz." | "fluid ounce" | "fluid ounces" => {
            return "fl oz".to_string()
        }
        "to taste" => return "to taste".to_string(),
        _ => {}
    }
    // Strip trailing 's' for simple plurals (cups→cup, grams→gram, tbsps→tbsp)
    // But not for units that naturally end in 's'
    let no_plural = if s.ends_with('s') && s != "to taste" && s != "fl oz" && s.len() > 2 {
        &s[..s.len() - 1]
    } else {
        &s
    };
    // Normalize common aliases
    match no_plural {
        "gram" | "g" => "g".to_string(),
        "kilogram" | "kg" => "kg".to_string(),
        "milligram" | "mg" => "mg".to_string(),
        "ounce" | "oz" => "oz".to_string(),
        "pound" | "lb" => "lb".to_string(),
        "milliliter" | "millilitre" | "ml" => "ml".to_string(),
        "liter" | "litre" | "l" => "l".to_string(),
        "cup" => "cup".to_string(),
        "tablespoon" | "tbsp" | "tbs" | "tb" => "tbsp".to_string(),
        "teaspoon" | "tsp" | "ts" => "tsp".to_string(),
        "pint" | "pt" => "pint".to_string(),
        "quart" | "qt" => "quart".to_string(),
        "gallon" | "gal" => "gallon".to_string(),
        other => other.to_string(),
    }
}

// --- Weight conversions (to grams) ---

fn weight_to_grams(normalized: &str) -> Option<f64> {
    match normalized {
        "g" => Some(1.0),
        "kg" => Some(1000.0),
        "oz" => Some(28.3495),
        "lb" => Some(453.592),
        "mg" => Some(0.001),
        _ => None,
    }
}

fn best_weight_unit(grams: f64) -> (f64, String) {
    if grams >= 1000.0 {
        (grams / 1000.0, "kg".to_string())
    } else if grams < 1.0 {
        (grams * 1000.0, "mg".to_string())
    } else {
        (grams, "g".to_string())
    }
}

// --- Volume conversions (to ml) ---

fn volume_to_ml(normalized: &str) -> Option<f64> {
    match normalized {
        "ml" => Some(1.0),
        "l" => Some(1000.0),
        "cup" => Some(236.588),
        "tbsp" => Some(14.787),
        "tsp" => Some(4.929),
        "fl oz" => Some(29.574),
        "pint" => Some(473.176),
        "quart" => Some(946.353),
        "gallon" => Some(3785.41),
        _ => None,
    }
}

fn best_volume_unit(ml: f64) -> (f64, String) {
    // Prefer common cooking units over metric
    if ml >= 3785.0 {
        (ml / 3785.41, "gallon".to_string())
    } else if ml >= 946.0 {
        (ml / 946.353, "quart".to_string())
    } else if ml >= 236.0 {
        (ml / 236.588, "cup".to_string())
    } else if ml >= 14.0 {
        (ml / 14.787, "tbsp".to_string())
    } else if ml >= 4.0 {
        (ml / 4.929, "tsp".to_string())
    } else {
        (ml, "ml".to_string())
    }
}

// --- Shopping-list rounding ---

/// How a shopping list rounds a total measured in a given unit.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShoppingUnitClass {
    /// Whole items such as lemons, heads of garlic or cans. The total rounds up
    /// to a whole number.
    Count,
    /// Weights and grocery volumes. The total rounds up to the next quarter
    /// below 0.5, the next half below 2, and the next whole number from 2 up.
    Graduated,
    /// Small measurements and "to taste". The total is returned unchanged.
    PassThrough,
}

/// Returns true when the unit, after normalization, is a small measurement
/// such as a teaspoon, pinch or dash.
pub fn is_small_measurement_unit(unit: &str) -> bool {
    // `normalize_unit` strips only a trailing "s", so "pinches", "dashes" and
    // "splashes" arrive as "pinche", "dashe" and "splashe".
    matches!(
        normalize_unit(unit).as_str(),
        "tsp"
            | "tbsp"
            | "pinch"
            | "pinche"
            | "dash"
            | "dashe"
            | "splash"
            | "splashe"
            | "drop"
            | "shake"
    )
}

/// Classifies a unit for shopping-list rounding.
pub fn shopping_unit_class(unit: &str) -> ShoppingUnitClass {
    // Small measurements are checked first because tsp and tbsp are also
    // volume units.
    if is_small_measurement_unit(unit) || normalize_unit(unit) == "to taste" {
        ShoppingUnitClass::PassThrough
    } else if unit_category(unit).is_some() {
        ShoppingUnitClass::Graduated
    } else {
        // A unitless line counts whole items such as eggs or lemons, so it
        // rounds up like a head, bunch, can or any other unit the converter
        // does not measure.
        ShoppingUnitClass::Count
    }
}

/// Rounds a total up to an amount a shopper can buy, following the rule for
/// `class`. Returns the rounded value and whether rounding changed it.
///
/// Zero, negative and non-finite values are returned unchanged and reported
/// as not rounded.
pub fn round_up_for_shopping(value: f64, class: ShoppingUnitClass) -> (f64, bool) {
    if !value.is_finite() || value <= 0.0 {
        return (value, false);
    }
    match class {
        ShoppingUnitClass::Count => ceil_to_step(value, 1.0),
        ShoppingUnitClass::Graduated => {
            let step = if value < 0.5 {
                0.25
            } else if value < 2.0 {
                0.5
            } else {
                1.0
            };
            ceil_to_step(value, step)
        }
        ShoppingUnitClass::PassThrough => (value, false),
    }
}

// Measured in steps, not in the unit, so the tolerance holds for 1000 g as
// well as for 0.25 cup. Summed amounts carry float error such as
// 3.0000000000000004, and a plain ceil would buy a fourth item for it.
const SNAP_TOLERANCE_STEPS: f64 = 1e-6;

fn ceil_to_step(value: f64, step: f64) -> (f64, bool) {
    let raw = value / step;
    let nearest = raw.round();
    // Snapping to zero steps would round a tiny positive amount to nothing.
    let steps = if nearest >= 1.0 && (raw - nearest).abs() < SNAP_TOLERANCE_STEPS {
        nearest
    } else {
        raw
    };
    let ceiled = steps.ceil();
    (ceiled * step, ceiled != steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rounded(value: f64, class: ShoppingUnitClass) -> f64 {
        round_up_for_shopping(value, class).0
    }

    #[test]
    fn tiny_positive_values_round_up_to_one_step_instead_of_zero() {
        assert_eq!(
            round_up_for_shopping(1e-7, ShoppingUnitClass::Graduated),
            (0.25, true)
        );
        assert_eq!(
            round_up_for_shopping(1e-7, ShoppingUnitClass::Count),
            (1.0, true)
        );
    }

    #[test]
    fn small_measurement_units_include_es_plurals() {
        for unit in [
            "tsp",
            "tsps",
            "Tbsp",
            "tablespoons",
            "pinch",
            "pinches",
            "dash",
            "dashes",
            "splash",
            "splashes",
            "drops",
            "shakes",
        ] {
            assert!(is_small_measurement_unit(unit), "{unit} should be small");
        }
        for unit in ["cup", "oz", "lb", "g", "ml", "whole", "clove", ""] {
            assert!(
                !is_small_measurement_unit(unit),
                "{unit} should not be small"
            );
        }
    }

    #[test]
    fn small_measurements_and_to_taste_pass_through() {
        for unit in [
            "tsp",
            "Tbsp",
            "teaspoons",
            "pinch",
            "pinches",
            "dash",
            "to taste",
            "To Taste",
        ] {
            assert!(
                matches!(shopping_unit_class(unit), ShoppingUnitClass::PassThrough),
                "{unit} should pass through"
            );
        }
    }

    #[test]
    fn weights_and_grocery_volumes_are_graduated() {
        for unit in [
            "lb", "pounds", "oz", "g", "kg", "cup", "cups", "quart", "gallon", "liter", "pint",
            "fl oz", "ml",
        ] {
            assert!(
                matches!(shopping_unit_class(unit), ShoppingUnitClass::Graduated),
                "{unit} should be graduated"
            );
        }
    }

    #[test]
    fn unitless_and_other_units_are_counts() {
        for unit in [
            "", "each", "whole", "head", "heads", "clove", "cloves", "bunch", "bunches", "can",
        ] {
            assert!(
                matches!(shopping_unit_class(unit), ShoppingUnitClass::Count),
                "{unit:?} should be a count"
            );
        }
    }

    #[test]
    fn counts_round_up_to_a_whole_number() {
        let cases = [
            (0.88, 1.0),
            (2.4, 3.0),
            (3.0, 3.0),
            (2.999999, 3.0),
            (0.01, 1.0),
        ];
        for (input, expected) in cases {
            assert_eq!(
                rounded(input, ShoppingUnitClass::Count),
                expected,
                "{input}"
            );
        }
        assert_eq!(rounded(1234.5, ShoppingUnitClass::Count), 1235.0);
    }

    #[test]
    fn graduated_values_already_on_a_step_are_unchanged() {
        for value in [0.25, 0.5, 1.0, 1.5, 2.0, 5.0] {
            assert_eq!(
                round_up_for_shopping(value, ShoppingUnitClass::Graduated),
                (value, false),
                "{value}"
            );
        }
    }

    #[test]
    fn graduated_values_near_each_threshold_round_up_by_their_tier() {
        let cases = [
            (0.24, 0.25),
            (0.26, 0.5),
            (0.49, 0.5),
            (0.51, 1.0),
            (1.99, 2.0),
            (2.01, 3.0),
        ];
        for (input, expected) in cases {
            assert_eq!(
                rounded(input, ShoppingUnitClass::Graduated),
                expected,
                "{input}"
            );
        }
    }

    #[test]
    fn graduated_rounding_matches_the_documented_examples() {
        assert_eq!(rounded(1.75, ShoppingUnitClass::Graduated), 2.0);
        assert_eq!(rounded(0.3, ShoppingUnitClass::Graduated), 0.5);
        assert_eq!(rounded(4.2, ShoppingUnitClass::Graduated), 5.0);
    }

    #[test]
    fn graduated_rounding_handles_very_small_and_very_large_values() {
        assert_eq!(rounded(0.001, ShoppingUnitClass::Graduated), 0.25);
        assert_eq!(rounded(9999.2, ShoppingUnitClass::Graduated), 10000.0);
    }

    // Each input below rounds one step too high when the snap is removed.
    #[test]
    fn float_error_does_not_round_up_an_extra_step() {
        assert_eq!(rounded((0.1 + 0.2) * 10.0, ShoppingUnitClass::Count), 3.0);
        assert_eq!(
            rounded((0.1 + 0.2) / 1.2, ShoppingUnitClass::Graduated),
            0.25
        );
        assert_eq!(
            rounded((0.1 + 0.2) * 5.0, ShoppingUnitClass::Graduated),
            1.5
        );
        assert_eq!(
            rounded(1000.0 + 1e-12, ShoppingUnitClass::Graduated),
            1000.0
        );
    }

    #[test]
    fn rounded_flag_is_false_when_only_the_snap_moved_the_value() {
        assert_eq!(
            round_up_for_shopping(2.0000000001, ShoppingUnitClass::Count),
            (2.0, false)
        );
        assert_eq!(
            round_up_for_shopping(1.5000000000000002, ShoppingUnitClass::Graduated),
            (1.5, false)
        );
        assert_eq!(
            round_up_for_shopping(2.4, ShoppingUnitClass::Count),
            (3.0, true)
        );
        assert_eq!(
            round_up_for_shopping(0.3, ShoppingUnitClass::Graduated),
            (0.5, true)
        );
    }

    #[test]
    fn non_positive_and_non_finite_values_are_returned_unchanged() {
        for class in [ShoppingUnitClass::Count, ShoppingUnitClass::Graduated] {
            assert_eq!(round_up_for_shopping(-0.3, class), (-0.3, false));
            assert_eq!(round_up_for_shopping(0.0, class), (0.0, false));
            assert_eq!(
                round_up_for_shopping(f64::INFINITY, class),
                (f64::INFINITY, false)
            );
            let (nan, changed) = round_up_for_shopping(f64::NAN, class);
            assert!(nan.is_nan());
            assert!(!changed);
        }
    }

    #[test]
    fn pass_through_values_are_unchanged() {
        assert_eq!(
            round_up_for_shopping(0.875, ShoppingUnitClass::PassThrough),
            (0.875, false)
        );
        assert_eq!(
            round_up_for_shopping(2.3, ShoppingUnitClass::PassThrough),
            (2.3, false)
        );
    }

    #[test]
    fn graduated_rounding_never_decreases_and_never_rounds_down() {
        let mut previous = 0.0;
        for i in 1..=10_000 {
            let value = f64::from(i) * 0.001;
            let out = rounded(value, ShoppingUnitClass::Graduated);
            assert!(
                out >= previous,
                "{value} rounded to {out}, below {previous}"
            );
            assert!(out >= value - 1e-6, "{value} rounded down to {out}");
            previous = out;
        }
    }

    #[test]
    fn shopping_unit_class_serializes_in_snake_case() {
        let json = serde_json::to_string(&[
            ShoppingUnitClass::Count,
            ShoppingUnitClass::Graduated,
            ShoppingUnitClass::PassThrough,
        ])
        .unwrap();
        assert_eq!(json, r#"["count","graduated","pass_through"]"#);
    }

    #[test]
    fn test_normalize_unit() {
        assert_eq!(normalize_unit("cups"), "cup");
        assert_eq!(normalize_unit("Tbsp"), "tbsp");
        assert_eq!(normalize_unit("GRAMS"), "g");
        assert_eq!(normalize_unit("fl oz"), "fl oz");
        assert_eq!(normalize_unit("to taste"), "to taste");
        assert_eq!(normalize_unit("whole"), "whole");
    }

    #[test]
    fn test_unit_category() {
        assert_eq!(unit_category("cups"), Some("volume"));
        assert_eq!(unit_category("grams"), Some("weight"));
        assert_eq!(unit_category("oz"), Some("weight"));
        assert_eq!(unit_category("whole"), None);
        assert_eq!(unit_category("pinch"), None);
    }

    #[test]
    fn test_to_base_and_back() {
        // 2 cups → ml → cups
        let base = to_base(2.0, "cups").unwrap();
        let back = from_base(base, "cup").unwrap();
        assert!((back - 2.0).abs() < 0.001);

        // 1 lb → grams → lb
        let base = to_base(1.0, "lb").unwrap();
        assert!((base - 453.592).abs() < 0.01);
        let back = from_base(base, "lb").unwrap();
        assert!((back - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_best_display_unit() {
        let (val, unit) = best_display_unit(500.0, "weight");
        assert_eq!(unit, "g");
        assert!((val - 500.0).abs() < 0.001);

        let (val, unit) = best_display_unit(2000.0, "weight");
        assert_eq!(unit, "kg");
        assert!((val - 2.0).abs() < 0.001);

        let (_val, unit) = best_display_unit(473.176, "volume");
        assert_eq!(unit, "cup");
    }
}
