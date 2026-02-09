use serde::{Deserialize, Serialize};

use crate::commands::recipe::{IngredientAmountDto, IngredientDto};
use crate::services::unit_converter;

/// An ingredient that scaled to a fractional amount for a discrete unit (e.g., 2.25 eggs).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FlaggedIngredient {
    pub index: usize,
    pub name: String,
    pub scaled_value: f64,
    pub unit: String,
}

/// Result of scaling a recipe's ingredients.
#[derive(Debug, Serialize, Deserialize)]
pub struct ScaleResult {
    pub ingredients: Vec<IngredientDto>,
    pub flagged: Vec<FlaggedIngredient>,
}

/// Returns true if a unit represents a discrete/indivisible quantity.
fn is_discrete_unit(unit: &str) -> bool {
    let normalized = unit_converter::normalize_unit(unit);
    // Discrete if not a recognized weight/volume unit
    unit_converter::unit_category(&normalized).is_none() && !normalized.is_empty()
}

/// Scale ingredients by a ratio and flag any discrete units with fractional results.
pub fn scale_ingredients(ingredients: &[IngredientDto], ratio: f64) -> ScaleResult {
    let mut scaled = Vec::with_capacity(ingredients.len());
    let mut flagged = Vec::new();

    for (i, ing) in ingredients.iter().enumerate() {
        let new_amount = scale_amount(&ing.amount, ratio);
        let scaled_ing = IngredientDto {
            name: ing.name.clone(),
            amount: new_amount.clone(),
            unit: ing.unit.clone(),
            notes: ing.notes.clone(),
        };

        // Flag discrete units with fractional amounts
        if is_discrete_unit(&ing.unit) {
            let value = primary_value(&new_amount);
            if value.fract() != 0.0 {
                flagged.push(FlaggedIngredient {
                    index: i,
                    name: ing.name.clone(),
                    scaled_value: value,
                    unit: ing.unit.clone(),
                });
            }
        }

        scaled.push(scaled_ing);
    }

    ScaleResult {
        ingredients: scaled,
        flagged,
    }
}

fn scale_amount(amount: &IngredientAmountDto, ratio: f64) -> IngredientAmountDto {
    match amount {
        IngredientAmountDto::Single { value } => IngredientAmountDto::Single {
            value: round_to_2(value * ratio),
        },
        IngredientAmountDto::Range { min, max } => IngredientAmountDto::Range {
            min: round_to_2(min * ratio),
            max: round_to_2(max * ratio),
        },
    }
}

/// Extract the primary value for flagging purposes (Single → value, Range → min).
fn primary_value(amount: &IngredientAmountDto) -> f64 {
    match amount {
        IngredientAmountDto::Single { value } => *value,
        IngredientAmountDto::Range { min, .. } => *min,
    }
}

fn round_to_2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ingredient(name: &str, value: f64, unit: &str) -> IngredientDto {
        IngredientDto {
            name: name.to_string(),
            amount: IngredientAmountDto::Single { value },
            unit: unit.to_string(),
            notes: None,
        }
    }

    #[test]
    fn scale_up_simple() {
        let ingredients = vec![
            make_ingredient("flour", 2.0, "cups"),
            make_ingredient("salt", 1.0, "tsp"),
        ];
        let result = scale_ingredients(&ingredients, 1.5);
        match &result.ingredients[0].amount {
            IngredientAmountDto::Single { value } => assert_eq!(*value, 3.0),
            _ => panic!("expected Single"),
        }
        match &result.ingredients[1].amount {
            IngredientAmountDto::Single { value } => assert_eq!(*value, 1.5),
            _ => panic!("expected Single"),
        }
        assert!(result.flagged.is_empty());
    }

    #[test]
    fn scale_down_simple() {
        let ingredients = vec![make_ingredient("flour", 4.0, "cups")];
        let result = scale_ingredients(&ingredients, 0.5);
        match &result.ingredients[0].amount {
            IngredientAmountDto::Single { value } => assert_eq!(*value, 2.0),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn flags_fractional_discrete_units() {
        let ingredients = vec![
            make_ingredient("eggs", 3.0, "whole"),
            make_ingredient("flour", 2.0, "cups"),
        ];
        // Scale 4 servings → 6 servings (ratio 1.5)
        let result = scale_ingredients(&ingredients, 1.5);
        assert_eq!(result.flagged.len(), 1);
        assert_eq!(result.flagged[0].name, "eggs");
        assert_eq!(result.flagged[0].scaled_value, 4.5);
    }

    #[test]
    fn no_flag_for_whole_discrete_amounts() {
        let ingredients = vec![make_ingredient("eggs", 2.0, "whole")];
        // 2 * 2.0 = 4.0 — whole number, no flag
        let result = scale_ingredients(&ingredients, 2.0);
        assert!(result.flagged.is_empty());
    }

    #[test]
    fn scales_range_amounts() {
        let ingredients = vec![IngredientDto {
            name: "garlic".to_string(),
            amount: IngredientAmountDto::Range {
                min: 2.0,
                max: 3.0,
            },
            unit: "clove".to_string(),
            notes: None,
        }];
        let result = scale_ingredients(&ingredients, 2.0);
        match &result.ingredients[0].amount {
            IngredientAmountDto::Range { min, max } => {
                assert_eq!(*min, 4.0);
                assert_eq!(*max, 6.0);
            }
            _ => panic!("expected Range"),
        }
    }

    #[test]
    fn flags_range_with_fractional_discrete() {
        let ingredients = vec![IngredientDto {
            name: "garlic".to_string(),
            amount: IngredientAmountDto::Range {
                min: 2.0,
                max: 3.0,
            },
            unit: "clove".to_string(),
            notes: None,
        }];
        // 2 * 1.5 = 3.0, but min is 2*1.5=3.0 — no flag. Let's use 1.3
        let result = scale_ingredients(&ingredients, 1.3);
        // min = 2.6, max = 3.9 → flagged because min is fractional
        assert_eq!(result.flagged.len(), 1);
        assert_eq!(result.flagged[0].scaled_value, 2.6);
    }

    #[test]
    fn is_discrete_unit_detection() {
        assert!(is_discrete_unit("whole"));
        assert!(is_discrete_unit("piece"));
        assert!(is_discrete_unit("clove"));
        assert!(is_discrete_unit("to taste"));
        assert!(!is_discrete_unit("cups"));
        assert!(!is_discrete_unit("g"));
        assert!(!is_discrete_unit("tbsp"));
        assert!(!is_discrete_unit("oz"));
    }

    #[test]
    fn rounding_precision() {
        let ingredients = vec![make_ingredient("flour", 1.0, "cups")];
        // 1.0 * (1.0/3.0) = 0.333...
        let result = scale_ingredients(&ingredients, 1.0 / 3.0);
        match &result.ingredients[0].amount {
            IngredientAmountDto::Single { value } => assert_eq!(*value, 0.33),
            _ => panic!("expected Single"),
        }
    }
}
