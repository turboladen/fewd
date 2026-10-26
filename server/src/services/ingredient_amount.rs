//! Re-exports the amount parser + unit predicate from the migration crate
//! and adapts the kind enum to the server's `IngredientAmountDto`.
//!
//! Canonical implementation lives in `migration::ingredient_amount` so the
//! runtime parser and the backfill migration share one source of truth.

pub use migration::{
    is_known_unit, try_parse_amount, unicode_fraction_value, AmountKind, DISCRETE_UNITS,
    MEASURED_UNITS, RANGE_DASHES,
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
    // normalizer, but the converter keeps its own conversion tables, and the
    // unit lists must agree with them.
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

    // Every weight and volume spelling the converter classifies must also
    // be a known unit to the parser.
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
            assert!(is_known_unit(unit), "is_known_unit must accept {unit:?}");
            assert!(
                unit_converter::unit_category(unit).is_some(),
                "unit_converter::unit_category must classify {unit:?}"
            );
        }
    }
}
