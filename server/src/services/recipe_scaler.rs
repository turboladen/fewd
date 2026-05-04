use serde::{Deserialize, Serialize};

use crate::dto::{IngredientAmountDto, IngredientDto};
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
            prep: ing.prep.clone(),
            amount: new_amount.clone(),
            unit: ing.unit.clone(),
            notes: ing.notes.clone(),
            // Recursively scale the alternative so a 2x recipe scales BOTH
            // "8 flour tortillas" and "10 corn tortillas". Flagging stays
            // primary-only — surfacing fractional alternatives as separate
            // rows would clutter the UI for what is conceptually one
            // ingredient slot.
            or_alternative: ing
                .or_alternative
                .as_deref()
                .map(|alt| Box::new(scale_one(alt, ratio))),
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

/// Scale a single ingredient (used recursively for `or_alternative`).
/// Does not flag — flagging is intentionally primary-only at the top-level
/// `scale_ingredients` boundary.
fn scale_one(ing: &IngredientDto, ratio: f64) -> IngredientDto {
    IngredientDto {
        name: ing.name.clone(),
        prep: ing.prep.clone(),
        amount: scale_amount(&ing.amount, ratio),
        unit: ing.unit.clone(),
        notes: ing.notes.clone(),
        or_alternative: ing
            .or_alternative
            .as_deref()
            .map(|alt| Box::new(scale_one(alt, ratio))),
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
            prep: None,
            amount: IngredientAmountDto::Single { value },
            unit: unit.to_string(),
            notes: None,
            or_alternative: None,
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
            prep: None,
            amount: IngredientAmountDto::Range { min: 2.0, max: 3.0 },
            unit: "clove".to_string(),
            notes: None,
            or_alternative: None,
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
            prep: None,
            amount: IngredientAmountDto::Range { min: 2.0, max: 3.0 },
            unit: "clove".to_string(),
            notes: None,
            or_alternative: None,
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

    #[test]
    fn scales_or_alternative_recursively_and_never_flags_alts() {
        // Primary: 8 flour tortillas (whole, discrete) → at 2x = 16 (no flag)
        // Alt: 10 corn tortillas (whole, discrete) → at 2x must also become 20
        // Chained alt: 0.5 cups water (cups, non-discrete) → at 2x = 1.0 cups
        //
        // Two invariants: (1) every level of the chain scales by the same
        // ratio; (2) discrete-fractional flagging is primary-only — a chain
        // member with a fractional discrete amount must NOT add a flag row,
        // since flags index back into `ingredients` by primary position only.
        let chained_alt = IngredientDto {
            name: "water".to_string(),
            prep: None,
            amount: IngredientAmountDto::Single { value: 0.5 },
            unit: "cups".to_string(),
            notes: None,
            or_alternative: None,
        };
        let alt = IngredientDto {
            name: "corn tortillas".to_string(),
            prep: None,
            amount: IngredientAmountDto::Single { value: 10.0 },
            unit: "whole".to_string(),
            notes: None,
            or_alternative: Some(Box::new(chained_alt)),
        };
        let primary = IngredientDto {
            name: "flour tortillas".to_string(),
            prep: None,
            amount: IngredientAmountDto::Single { value: 8.0 },
            unit: "whole".to_string(),
            notes: None,
            or_alternative: Some(Box::new(alt)),
        };
        let result = scale_ingredients(&[primary], 2.0);

        // Primary scaled.
        match &result.ingredients[0].amount {
            IngredientAmountDto::Single { value } => assert_eq!(*value, 16.0),
            _ => panic!("expected Single"),
        }
        // Depth-1 alt scaled.
        let alt = result.ingredients[0]
            .or_alternative
            .as_ref()
            .expect("alt present");
        match &alt.amount {
            IngredientAmountDto::Single { value } => assert_eq!(*value, 20.0),
            _ => panic!("expected Single"),
        }
        // Depth-2 alt scaled too.
        let chained = alt.or_alternative.as_ref().expect("chained alt present");
        match &chained.amount {
            IngredientAmountDto::Single { value } => assert_eq!(*value, 1.0),
            _ => panic!("expected Single"),
        }
        // Flagging is primary-only — the alt's would-be-flag-eligible state
        // is intentionally invisible to the flagged list.
        assert!(result.flagged.is_empty());
    }

    #[test]
    fn fractional_alt_does_not_emit_flag() {
        // 3 eggs primary → 4.5 (flagged), alt 1 cup egg substitute → 1.5
        // (a fractional cup is fine — non-discrete unit). Even if a chained
        // alt did land on a fractional discrete unit, the flagged Vec must
        // still contain a single entry pointing at the primary, never the
        // alt.
        let alt = IngredientDto {
            name: "egg substitute".to_string(),
            prep: None,
            amount: IngredientAmountDto::Single { value: 1.0 },
            unit: "cup".to_string(),
            notes: None,
            or_alternative: None,
        };
        let primary = IngredientDto {
            name: "eggs".to_string(),
            prep: None,
            amount: IngredientAmountDto::Single { value: 3.0 },
            unit: "whole".to_string(),
            notes: None,
            or_alternative: Some(Box::new(alt)),
        };
        let result = scale_ingredients(&[primary], 1.5);
        assert_eq!(result.flagged.len(), 1);
        assert_eq!(result.flagged[0].name, "eggs");
        assert_eq!(result.flagged[0].scaled_value, 4.5);
    }
}
