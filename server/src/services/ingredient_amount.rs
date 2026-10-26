//! Re-exports the amount parser + unit predicate from the migration crate
//! and adapts the kind enum to the server's `IngredientAmountDto`.
//!
//! Canonical implementation lives in `migration::ingredient_amount` so the
//! runtime parser and the backfill migration share one source of truth.

pub use migration::{
    is_known_unit, try_parse_amount, unicode_fraction_value, AmountKind, DISCRETE_UNITS,
    MEASURED_UNITS,
};

use crate::dto::IngredientAmountDto;

/// Parse an amount token directly into the server's DTO shape. Returns
/// `None` on garbage; callers fall back to the "no parseable amount" branch.
pub fn try_parse_amount_dto(s: &str) -> Option<IngredientAmountDto> {
    match try_parse_amount(s)? {
        AmountKind::Single(value) => Some(IngredientAmountDto::Single { value }),
        AmountKind::Range { min, max } => Some(IngredientAmountDto::Range { min, max }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::unit_converter;

    // Drift guard: `is_known_unit` and `unit_converter` share one
    // normalizer, but the converter keeps its own conversion tables. Every
    // spelling the converter classifies as a weight or volume must be a
    // known unit, and the unit lists must agree with those tables.
    #[test]
    fn unit_lists_match_unit_converter_categories() {
        for unit in MEASURED_UNITS {
            assert!(
                unit_converter::unit_category(unit).is_some(),
                "unit_converter must classify measured unit {unit:?}"
            );
        }
        for unit in DISCRETE_UNITS {
            assert!(
                unit_converter::unit_category(unit).is_none(),
                "unit_converter must not convert discrete unit {unit:?}"
            );
        }
    }

    #[test]
    fn is_known_unit_covers_unit_converter_weight_and_volume() {
        let units = [
            // Weight (per `unit_converter::weight_to_grams`)
            "g",
            "gram",
            "grams",
            "kg",
            "kilogram",
            "kilograms",
            "mg",
            "milligram",
            "milligrams",
            "oz",
            "ounce",
            "ounces",
            "lb",
            "pound",
            "pounds",
            // Volume (per `unit_converter::volume_to_ml`)
            "ml",
            "milliliter",
            "millilitre",
            "milliliters",
            "millilitres",
            "l",
            "liter",
            "litre",
            "liters",
            "litres",
            "cup",
            "cups",
            "tbsp",
            "tablespoon",
            "tablespoons",
            "tbs",
            "tb",
            "tsp",
            "teaspoon",
            "teaspoons",
            "ts",
            "pint",
            "pints",
            "pt",
            "quart",
            "quarts",
            "qt",
            "gallon",
            "gallons",
            "gal",
            "fl oz",
        ];
        for unit in units {
            assert!(
                is_known_unit(unit),
                "is_known_unit must accept {unit:?} (normalize_unit recognizes it)"
            );
            // Sanity: unit_converter must also classify it (catches us
            // adding to is_known_unit's list without telling unit_converter
            // — which would silently break shopping aggregation).
            assert!(
                unit_converter::unit_category(unit).is_some(),
                "unit_converter::unit_category must classify {unit:?}"
            );
        }
    }
}
