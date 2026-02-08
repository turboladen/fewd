//! Cooking unit converter for shopping list aggregation.
//!
//! Converts between weight units (base: grams) and volume units (base: ml).
//! Discrete/uncategorized units (whole, piece, etc.) are not converted.

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
    let no_plural = if s.ends_with('s')
        && s != "to taste"
        && s != "fl oz"
        && s.len() > 2
    {
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

#[cfg(test)]
mod tests {
    use super::*;

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

        let (val, unit) = best_display_unit(473.176, "volume");
        assert_eq!(unit, "cup");
    }
}
