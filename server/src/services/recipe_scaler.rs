use serde::{Deserialize, Serialize};

use crate::dto::{IngredientAmountDto, IngredientDto};
use crate::services::unit_converter;

/// An ingredient whose discrete amount the scale preview rounded to a whole
/// number (e.g., 3.75 eggs shown as 4). `ScaleResult::ingredients[index]`
/// holds the rounded amount, which the user may still override.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FlaggedIngredient {
    pub index: usize,
    pub name: String,
    /// Reports the primary amount, a range's min, scaled and rounded to six
    /// decimals but not yet to a whole number.
    pub scaled_value: f64,
    pub unit: String,
}

/// Result of scaling a recipe's ingredients.
#[derive(Debug, Serialize, Deserialize)]
pub struct ScaleResult {
    pub ingredients: Vec<IngredientDto>,
    pub flagged: Vec<FlaggedIngredient>,
}

/// Returns true if the scale preview rounds amounts in this unit to whole
/// numbers: any unit that is neither a weight nor a volume, including no unit
/// at all, except "to taste".
fn rounds_to_whole(unit: &str) -> bool {
    let normalized = unit_converter::normalize_unit(unit);
    // AI imports and MCP writes store counts such as "3 eggs" with unit "", so
    // an empty unit counts whole items. "To taste" is a seasoning cue rather
    // than a count, and rounding it would fire the banner on nearly every scale.
    normalized != "to taste" && unit_converter::unit_category(&normalized).is_none()
}

/// Scale ingredients by a ratio for a preview the user reviews before saving.
///
/// Amounts round to two decimals, except that discrete units (such as eggs or
/// cloves) round to the nearest whole number, half away from zero, and a
/// positive amount never rounds below 1. A range whose bounds round to the
/// same number becomes a single amount. Alternatives round the same way.
/// A primary ingredient whose rounding changed its amount is listed in
/// `flagged`; alternatives are never flagged.
pub fn scale_ingredients(ingredients: &[IngredientDto], ratio: f64) -> ScaleResult {
    let mut scaled = Vec::with_capacity(ingredients.len());
    let mut flagged = Vec::new();

    for (i, ing) in ingredients.iter().enumerate() {
        let (scaled_ing, rounded) = scale_one_for_preview(ing, ratio);
        // A bound changed when its whole number differs from the bound at six
        // decimals, and the clamp to 1 counts as a change. A single amount is
        // flagged when its value changed; a range, including one that collapsed
        // to a single amount, is flagged when either bound changed. So a range
        // whose max alone changed is flagged while `scaled_value`, which reports
        // only the min, can equal the rounded min. Flags index into
        // `ingredients` by primary position and the preview UI renders
        // alternatives read-only, so only a primary gets a flag.
        if rounded {
            flagged.push(FlaggedIngredient {
                index: i,
                name: ing.name.clone(),
                // Six decimals match the precision the rounding decision used,
                // so 1.495 is reported as 1.495 rather than as 1.5 beside a 1.
                scaled_value: round_to_6(primary_value(&ing.amount) * ratio),
                unit: ing.unit.clone(),
            });
        }
        scaled.push(scaled_ing);
    }

    ScaleResult {
        ingredients: scaled,
        flagged,
    }
}

/// Scale ingredients by a ratio, rounding every amount to two decimals.
///
/// Unlike [`scale_ingredients`], this never rounds discrete units to whole
/// numbers, so 3 eggs at 1.5x scale to 4.5.
pub fn scale_amounts(ingredients: &[IngredientDto], ratio: f64) -> Vec<IngredientDto> {
    ingredients
        .iter()
        .map(|ing| scale_one(ing, ratio))
        .collect()
}

/// Scale a single ingredient and its `or_alternative` chain to two decimals.
/// Does not flag.
// `scale_amounts` backs the servings-only rescale, which must keep discrete
// counts fractional, so whole-number rounding lives in `scale_one_for_preview`.
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

/// Scale a single ingredient and its `or_alternative` chain for the preview,
/// rounding discrete units to whole numbers. Returns whether that rounding
/// changed this ingredient's own amount.
fn scale_one_for_preview(ing: &IngredientDto, ratio: f64) -> (IngredientDto, bool) {
    let (amount, rounded) = if rounds_to_whole(&ing.unit) {
        round_discrete(&ing.amount, ratio)
    } else {
        (scale_amount(&ing.amount, ratio), false)
    };
    let scaled = IngredientDto {
        name: ing.name.clone(),
        prep: ing.prep.clone(),
        amount,
        unit: ing.unit.clone(),
        notes: ing.notes.clone(),
        or_alternative: ing
            .or_alternative
            .as_deref()
            .map(|alt| Box::new(scale_one_for_preview(alt, ratio).0)),
    };
    (scaled, rounded)
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

/// Scale a discrete amount to whole numbers. Returns whether any bound
/// changed; a collapsed range counts as changed only when a bound did.
fn round_discrete(amount: &IngredientAmountDto, ratio: f64) -> (IngredientAmountDto, bool) {
    match amount {
        IngredientAmountDto::Single { value } => {
            let (value, changed) = whole(value * ratio);
            (IngredientAmountDto::Single { value }, changed)
        }
        IngredientAmountDto::Range { min, max } => {
            let (min, min_changed) = whole(min * ratio);
            let (max, max_changed) = whole(max * ratio);
            let amount = if min == max {
                IngredientAmountDto::Single { value: min }
            } else {
                IngredientAmountDto::Range { min, max }
            };
            (amount, min_changed || max_changed)
        }
    }
}

/// Round a raw scaled bound to a whole number, clamping a positive bound to
/// at least 1. Returns the whole number and whether it differs from the
/// bound at six decimals.
fn whole(raw: f64) -> (f64, bool) {
    // Rounding to six decimals first absorbs float noise: 0.7 * (15/7) lands
    // a hair below 1.5 and must still round up. Rounding the two-decimal
    // value instead would push 0.495 to 0.5 and round it up wrongly.
    let exact = round_to_6(raw);
    let rounded = exact.round();
    // A positive amount means the ingredient is in the recipe, so rounding it
    // to 0 would silently drop it from the dish and the shopping list. The
    // clamp checks the raw product, which stays positive even where six
    // decimals round it to 0. The clamp does not survive a round trip: 1 egg
    // scaled from 4 servings to 1 previews as 1, and saving that and scaling
    // back to 4 previews 4 eggs. The row is flagged, so the user sees it.
    let value = if rounded == 0.0 && raw > 0.0 {
        1.0
    } else {
        rounded
    };
    (value, value != exact)
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

fn round_to_6(v: f64) -> f64 {
    (v * 1_000_000.0).round() / 1_000_000.0
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

    fn make_range(name: &str, min: f64, max: f64, unit: &str) -> IngredientDto {
        IngredientDto {
            amount: IngredientAmountDto::Range { min, max },
            ..make_ingredient(name, 0.0, unit)
        }
    }

    fn single(amount: &IngredientAmountDto) -> f64 {
        match amount {
            IngredientAmountDto::Single { value } => *value,
            IngredientAmountDto::Range { .. } => panic!("expected Single, got {amount:?}"),
        }
    }

    fn range(amount: &IngredientAmountDto) -> (f64, f64) {
        match amount {
            IngredientAmountDto::Range { min, max } => (*min, *max),
            IngredientAmountDto::Single { .. } => panic!("expected Range, got {amount:?}"),
        }
    }

    #[test]
    fn scale_up_simple() {
        let ingredients = vec![
            make_ingredient("flour", 2.0, "cups"),
            make_ingredient("salt", 1.0, "tsp"),
        ];
        let result = scale_ingredients(&ingredients, 1.5);
        assert_eq!(single(&result.ingredients[0].amount), 3.0);
        assert_eq!(single(&result.ingredients[1].amount), 1.5);
        assert!(result.flagged.is_empty());
    }

    #[test]
    fn scale_down_simple() {
        let ingredients = vec![make_ingredient("flour", 4.0, "cups")];
        let result = scale_ingredients(&ingredients, 0.5);
        assert_eq!(single(&result.ingredients[0].amount), 2.0);
    }

    #[test]
    fn rounds_discrete_amounts_to_nearest_whole_and_flags_them() {
        // 3 eggs from 4 to 5 servings is 3.75, which rounds to 4.
        let ingredients = vec![
            make_ingredient("eggs", 3.0, "whole"),
            make_ingredient("milk", 1.0, "cup"),
        ];
        let result = scale_ingredients(&ingredients, 5.0 / 4.0);
        assert_eq!(single(&result.ingredients[0].amount), 4.0);
        assert_eq!(single(&result.ingredients[1].amount), 1.25);
        assert_eq!(result.flagged.len(), 1);
        assert_eq!(result.flagged[0].index, 0);
        assert_eq!(result.flagged[0].name, "eggs");
        assert_eq!(result.flagged[0].scaled_value, 3.75);
    }

    #[test]
    fn rounds_half_away_from_zero() {
        let ingredients = vec![make_ingredient("eggs", 3.0, "whole")];
        let result = scale_ingredients(&ingredients, 1.5);
        assert_eq!(single(&result.ingredients[0].amount), 5.0);
        assert_eq!(result.flagged[0].scaled_value, 4.5);
    }

    #[test]
    fn a_tie_that_lands_below_one_and_a_half_in_floating_point_still_rounds_up() {
        let ratio: f64 = 15.0 / 7.0;
        // Rounding the raw product directly would give 1.
        assert_eq!((0.7 * ratio).round(), 1.0, "precondition");
        let result = scale_ingredients(&[make_ingredient("eggs", 0.7, "whole")], ratio);
        assert_eq!(single(&result.ingredients[0].amount), 2.0);
    }

    #[test]
    fn just_under_one_half_reaches_one_through_the_clamp() {
        // 0.99 * 0.5 = 0.495, which rounds to 0 and is clamped to 1. Rounding
        // the two-decimal value 0.5 instead would reach 1 for the wrong reason.
        let result = scale_ingredients(&[make_ingredient("eggs", 0.99, "whole")], 0.5);
        assert_eq!(single(&result.ingredients[0].amount), 1.0);
        assert_eq!(result.flagged.len(), 1);
    }

    #[test]
    fn just_under_one_and_a_half_rounds_down_to_one() {
        // 2.99 * 0.5 = 1.495 must round to 1, not to 2 through 1.5.
        let result = scale_ingredients(&[make_ingredient("eggs", 2.99, "whole")], 0.5);
        assert_eq!(single(&result.ingredients[0].amount), 1.0);
        // The flag reports the value the decision used, not a two-decimal 1.5.
        assert_eq!(result.flagged[0].scaled_value, 1.495);
    }

    #[test]
    fn value_just_off_a_whole_number_at_six_decimals_is_flagged() {
        // 1.002 * 2 = 2.004 differs from 2 at six decimals.
        let result = scale_ingredients(&[make_ingredient("eggs", 1.002, "whole")], 2.0);
        assert_eq!(single(&result.ingredients[0].amount), 2.0);
        assert_eq!(result.flagged.len(), 1);
        assert_eq!(result.flagged[0].scaled_value, 2.004);

        // 0.3333 * 3 = 0.9999, which rounds to 1 and is still flagged.
        let result = scale_ingredients(&[make_ingredient("eggs", 0.3333, "whole")], 3.0);
        assert_eq!(single(&result.ingredients[0].amount), 1.0);
        assert_eq!(result.flagged.len(), 1);
        assert_eq!(result.flagged[0].scaled_value, 0.9999);
    }

    #[test]
    fn positive_amount_that_rounds_to_zero_clamps_to_one() {
        let result = scale_ingredients(&[make_ingredient("eggs", 1.0, "whole")], 0.25);
        assert_eq!(single(&result.ingredients[0].amount), 1.0);
        assert_eq!(result.flagged.len(), 1);
        assert_eq!(result.flagged[0].scaled_value, 0.25);
    }

    #[test]
    fn zero_amount_stays_zero_and_is_not_flagged() {
        let result = scale_ingredients(&[make_ingredient("eggs", 0.0, "whole")], 1.5);
        assert_eq!(single(&result.ingredients[0].amount), 0.0);
        assert!(result.flagged.is_empty());
    }

    #[test]
    fn product_within_float_noise_of_a_whole_number_is_not_flagged() {
        let ingredients = vec![make_ingredient("eggs", 4.000_000_000_1, "whole")];
        let result = scale_ingredients(&ingredients, 1.0);
        assert_eq!(single(&result.ingredients[0].amount), 4.0);
        assert!(result.flagged.is_empty());
    }

    #[test]
    fn no_flag_for_whole_discrete_amounts() {
        let ingredients = vec![make_ingredient("eggs", 2.0, "whole")];
        let result = scale_ingredients(&ingredients, 2.0);
        assert_eq!(single(&result.ingredients[0].amount), 4.0);
        assert!(result.flagged.is_empty());
    }

    #[test]
    fn to_taste_scales_to_two_decimals_without_rounding_or_flagging() {
        let down = scale_ingredients(&[make_ingredient("salt", 1.0, "to taste")], 0.25);
        assert_eq!(single(&down.ingredients[0].amount), 0.25);
        assert!(down.flagged.is_empty());

        let up = scale_ingredients(&[make_ingredient("salt", 1.0, "to taste")], 1.5);
        assert_eq!(single(&up.ingredients[0].amount), 1.5);
        assert!(up.flagged.is_empty());
    }

    #[test]
    fn scales_continuous_range_amounts() {
        let result = scale_ingredients(&[make_range("milk", 1.0, 1.5, "cup")], 1.25);
        assert_eq!(range(&result.ingredients[0].amount), (1.25, 1.88));
        assert!(result.flagged.is_empty());
    }

    #[test]
    fn whole_discrete_range_is_not_flagged() {
        let result = scale_ingredients(&[make_range("garlic", 2.0, 3.0, "clove")], 2.0);
        assert_eq!(range(&result.ingredients[0].amount), (4.0, 6.0));
        assert!(result.flagged.is_empty());
    }

    #[test]
    fn rounds_both_bounds_of_a_discrete_range() {
        // 2.6-3.9 rounds to 3-4.
        let result = scale_ingredients(&[make_range("garlic", 2.0, 3.0, "clove")], 1.3);
        assert_eq!(range(&result.ingredients[0].amount), (3.0, 4.0));
        assert_eq!(result.flagged.len(), 1);
        assert_eq!(result.flagged[0].scaled_value, 2.6);
    }

    #[test]
    fn range_is_flagged_when_only_its_max_changed() {
        // 2-3 at 2.5x is 5-7.5; only the max rounds.
        let result = scale_ingredients(&[make_range("garlic", 2.0, 3.0, "clove")], 2.5);
        assert_eq!(range(&result.ingredients[0].amount), (5.0, 8.0));
        assert_eq!(result.flagged.len(), 1);
        assert_eq!(result.flagged[0].scaled_value, 5.0);
    }

    #[test]
    fn range_whose_bounds_round_together_becomes_single_and_is_flagged() {
        // 2-2.5 at 0.5x is 1-1.25, so the max changed.
        let result = scale_ingredients(&[make_range("garlic", 2.0, 2.5, "clove")], 0.5);
        assert_eq!(single(&result.ingredients[0].amount), 1.0);
        assert_eq!(result.flagged.len(), 1);

        // 2-3 at 0.3x is 0.6-0.9, so both bounds changed.
        let result = scale_ingredients(&[make_range("garlic", 2.0, 3.0, "clove")], 0.3);
        assert_eq!(single(&result.ingredients[0].amount), 1.0);
        assert_eq!(result.flagged.len(), 1);
    }

    #[test]
    fn degenerate_range_becomes_single_without_a_flag() {
        // Only the amount's type changes, so there is nothing to override.
        let result = scale_ingredients(&[make_range("garlic", 2.0, 2.0, "clove")], 2.0);
        assert_eq!(single(&result.ingredients[0].amount), 4.0);
        assert!(result.flagged.is_empty());
    }

    #[test]
    fn zero_range_bound_stays_zero_while_the_other_clamps() {
        // 0-2 at 0.3x is 0-0.6: the min stays 0 and the max clamps to 1.
        let result = scale_ingredients(&[make_range("garlic", 0.0, 2.0, "clove")], 0.3);
        assert_eq!(range(&result.ingredients[0].amount), (0.0, 1.0));
        assert_eq!(result.flagged.len(), 1);
    }

    #[test]
    fn unitless_amounts_round_like_counts() {
        let result = scale_ingredients(&[make_ingredient("eggs", 3.0, "")], 5.0 / 4.0);
        assert_eq!(single(&result.ingredients[0].amount), 4.0);
        assert_eq!(result.flagged.len(), 1);
        assert_eq!(result.flagged[0].scaled_value, 3.75);
    }

    #[test]
    fn unitless_zero_amount_stays_zero_and_is_not_flagged() {
        // AI imports store seasonings as amount 0 with unit "".
        let result = scale_ingredients(&[make_ingredient("salt", 0.0, "")], 5.0 / 4.0);
        assert_eq!(single(&result.ingredients[0].amount), 0.0);
        assert!(result.flagged.is_empty());
    }

    #[test]
    fn positive_product_that_six_decimals_round_to_zero_still_clamps_to_one() {
        let result = scale_ingredients(&[make_ingredient("eggs", 1.0, "whole")], 4e-7);
        assert_eq!(single(&result.ingredients[0].amount), 1.0);
        assert_eq!(result.flagged.len(), 1);
    }

    #[test]
    fn rounds_to_whole_detection() {
        assert!(rounds_to_whole("whole"));
        assert!(rounds_to_whole("piece"));
        assert!(rounds_to_whole("clove"));
        assert!(rounds_to_whole(""));
        assert!(rounds_to_whole("  "));
        assert!(!rounds_to_whole("to taste"));
        assert!(!rounds_to_whole("To Taste"));
        assert!(!rounds_to_whole("cups"));
        assert!(!rounds_to_whole("g"));
        assert!(!rounds_to_whole("tbsp"));
        assert!(!rounds_to_whole("oz"));
    }

    #[test]
    fn rounding_precision() {
        let ingredients = vec![make_ingredient("flour", 1.0, "cups")];
        let result = scale_ingredients(&ingredients, 1.0 / 3.0);
        assert_eq!(single(&result.ingredients[0].amount), 0.33);
    }

    #[test]
    fn rounds_every_level_of_an_alternative_chain_and_flags_only_the_primary() {
        // At 1.25x: 8 flour tortillas is 10 (whole, not flagged), 10 corn
        // tortillas is 12.5 and rounds to 13, and 0.5 cups of water is 0.63.
        let chained_alt = make_ingredient("water", 0.5, "cups");
        let alt = IngredientDto {
            or_alternative: Some(Box::new(chained_alt)),
            ..make_ingredient("corn tortillas", 10.0, "whole")
        };
        let primary = IngredientDto {
            or_alternative: Some(Box::new(alt)),
            ..make_ingredient("flour tortillas", 8.0, "whole")
        };
        let result = scale_ingredients(&[primary], 1.25);

        assert_eq!(single(&result.ingredients[0].amount), 10.0);
        let alt = result.ingredients[0]
            .or_alternative
            .as_ref()
            .expect("alt present");
        assert_eq!(single(&alt.amount), 13.0);
        let chained = alt.or_alternative.as_ref().expect("chained alt present");
        assert_eq!(single(&chained.amount), 0.63);
        // The alternative rounded, but flags index primaries only.
        assert!(result.flagged.is_empty());
    }

    #[test]
    fn rounds_a_discrete_alternative_two_levels_deep() {
        let chained_alt = make_ingredient("shallots", 3.0, "whole");
        let alt = IngredientDto {
            or_alternative: Some(Box::new(chained_alt)),
            ..make_ingredient("leek", 1.0, "cup")
        };
        let primary = IngredientDto {
            or_alternative: Some(Box::new(alt)),
            ..make_ingredient("onion", 1.0, "whole")
        };
        let result = scale_ingredients(&[primary], 1.5);

        assert_eq!(single(&result.ingredients[0].amount), 2.0);
        let alt = result.ingredients[0].or_alternative.as_ref().expect("alt");
        assert_eq!(single(&alt.amount), 1.5);
        let chained = alt.or_alternative.as_ref().expect("chained alt");
        assert_eq!(single(&chained.amount), 5.0);
        assert_eq!(result.flagged.len(), 1);
        assert_eq!(result.flagged[0].index, 0);
        assert_eq!(result.flagged[0].name, "onion");
    }

    #[test]
    fn scale_amounts_keeps_discrete_amounts_fractional() {
        let alt = make_ingredient("quail eggs", 5.0, "whole");
        let primary = IngredientDto {
            or_alternative: Some(Box::new(alt)),
            ..make_ingredient("eggs", 3.0, "whole")
        };
        let scaled = scale_amounts(&[primary, make_range("garlic", 2.0, 2.5, "clove")], 1.5);
        assert_eq!(single(&scaled[0].amount), 4.5);
        let alt = scaled[0].or_alternative.as_ref().expect("alt");
        assert_eq!(single(&alt.amount), 7.5);
        assert_eq!(range(&scaled[1].amount), (3.0, 3.75));
    }
}
