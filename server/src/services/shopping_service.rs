use std::collections::{HashMap, HashSet};

use sea_orm::*;

use crate::dto::PersonServingDto;
use crate::dto::{AggregatedIngredientDto, IngredientSourceDto, SourceType};
use crate::dto::{IngredientAmountDto, IngredientDto, ShoppingListSplitDto};
use crate::dto::{ShoppingAmountDto, ShoppingListItemDto};
use crate::entities::recipe::Entity as Recipe;
use crate::services::meal_service::MealService;
use crate::services::pantry_classifier;
use crate::services::unit_converter;

pub struct ShoppingService;

/// Intermediate struct to track where an ingredient came from
struct IngredientWithSource {
    ingredient: IngredientDto,
    source: IngredientSourceDto,
}

impl ShoppingService {
    pub async fn get_shopping_list(
        db: &DatabaseConnection,
        start_date: String,
        end_date: String,
    ) -> Result<Vec<AggregatedIngredientDto>, DbErr> {
        // 1. Fetch all meals in range
        let meals = MealService::get_all_for_date_range(db, start_date, end_date).await?;

        // 2. Collect all ingredients with source info
        let mut all_items: Vec<IngredientWithSource> = Vec::new();
        // Each bad stored row is logged once per list, not once per serving.
        let mut warned_meals: HashSet<&str> = HashSet::new();
        let mut warned_recipes: HashSet<String> = HashSet::new();

        for meal in &meals {
            let servings: Vec<PersonServingDto> =
                serde_json::from_str(&meal.servings).map_err(|e| {
                    DbErr::Custom(format!(
                        "Failed to parse servings for meal {}: {}",
                        meal.id, e
                    ))
                })?;

            let meal_date = meal.date.format("%Y-%m-%d").to_string();

            for serving in &servings {
                match serving {
                    PersonServingDto::Recipe {
                        recipe_id,
                        servings_count,
                        ..
                    } => {
                        // Writes reject such a count, but an older row may hold
                        // one. It asks for no food, so the serving is skipped.
                        if !servings_count.is_finite() || *servings_count <= 0.0 {
                            if warned_meals.insert(&meal.id) {
                                tracing::warn!(
                                    meal_id = %meal.id,
                                    servings_count,
                                    "skipping a stored meal serving with a non-positive servings_count"
                                );
                            }
                            continue;
                        }

                        let recipe = Recipe::find_by_id(recipe_id).one(db).await?;

                        if let Some(recipe) = recipe {
                            let ingredients: Vec<IngredientDto> =
                                serde_json::from_str(&recipe.ingredients).map_err(|e| {
                                    DbErr::Custom(format!(
                                        "Failed to parse ingredients for recipe {}: {}",
                                        recipe.id, e
                                    ))
                                })?;

                            // Writes reject a count below 1, but an older row may
                            // hold one. Dividing by it would make every amount
                            // infinite, so it is read as one serving, and the
                            // source reports the count the scale used.
                            let recipe_servings = if recipe.servings < 1 {
                                if warned_recipes.insert(recipe.id.clone()) {
                                    tracing::warn!(
                                        recipe_id = %recipe.id,
                                        servings = recipe.servings,
                                        "scaling a recipe with non-positive stored servings as one serving"
                                    );
                                }
                                1
                            } else {
                                recipe.servings
                            };
                            let scale = servings_count / f64::from(recipe_servings);

                            for ing in ingredients {
                                let scaled_amount = scale_amount(&ing.amount, scale);
                                let scaled_ingredient = IngredientDto {
                                    name: ing.name,
                                    prep: ing.prep,
                                    amount: scaled_amount.clone(),
                                    unit: ing.unit.clone(),
                                    notes: ing.notes,
                                    // Recursively scale the alternative chain
                                    // so primary and alternative stay in the
                                    // same serving scale — without this, a 2x
                                    // recipe would carry "16 flour tortillas"
                                    // primary alongside an unscaled "10 corn
                                    // tortillas" alt. Aggregator still groups
                                    // by primary `name` only; the data is
                                    // forwarded for downstream UX (fewd-ad0).
                                    or_alternative: ing
                                        .or_alternative
                                        .map(|alt| Box::new(scale_alt(*alt, scale))),
                                };

                                let source = IngredientSourceDto {
                                    amount: scaled_amount,
                                    unit: ing.unit,
                                    source_type: SourceType::Recipe,
                                    source_name: Some(recipe.name.clone()),
                                    meal_id: meal.id.clone(),
                                    meal_date: meal_date.clone(),
                                    meal_type: meal.meal_type.to_string(),
                                    recipe_servings: Some(recipe_servings),
                                    person_servings: Some(*servings_count),
                                };

                                all_items.push(IngredientWithSource {
                                    ingredient: scaled_ingredient,
                                    source,
                                });
                            }
                        }
                    }
                    PersonServingDto::Adhoc { adhoc_items, .. } => {
                        for ing in adhoc_items {
                            let source = IngredientSourceDto {
                                amount: ing.amount.clone(),
                                unit: ing.unit.clone(),
                                source_type: SourceType::Adhoc,
                                source_name: None,
                                meal_id: meal.id.clone(),
                                meal_date: meal_date.clone(),
                                meal_type: meal.meal_type.to_string(),
                                recipe_servings: None,
                                person_servings: None,
                            };

                            all_items.push(IngredientWithSource {
                                ingredient: ing.clone(),
                                source,
                            });
                        }
                    }
                }
            }
        }

        // 3. Merge sources from the same meal+recipe+ingredient into one line
        let all_items = merge_same_meal_sources(all_items);

        // 4. Group by ingredient name (case-insensitive)
        let mut groups: HashMap<String, Vec<IngredientWithSource>> = HashMap::new();
        for item in all_items {
            let key = item.ingredient.name.to_lowercase();
            groups.entry(key).or_default().push(item);
        }

        // 5. Aggregate each group
        let mut result: Vec<AggregatedIngredientDto> =
            groups.into_values().map(aggregate_group).collect();

        // 6. Sort alphabetically
        result.sort_by(|a, b| {
            a.ingredient_name
                .to_lowercase()
                .cmp(&b.ingredient_name.to_lowercase())
        });

        Ok(result)
    }

    /// Same date range as [`Self::get_shopping_list`], but partitions the
    /// flat list into `items_to_buy` and `pantry_staples_to_verify` using
    /// the pantry-staple classifier.
    ///
    /// Both partitions preserve the upstream alphabetical ordering, the
    /// `Range { min, max }` aggregation math, and the per-source breakdown
    /// in `AggregatedIngredientDto.items` — no shape changes versus the
    /// flat method, just the partition.
    pub async fn get_shopping_list_split(
        db: &DatabaseConnection,
        start_date: String,
        end_date: String,
    ) -> Result<ShoppingListSplitDto, DbErr> {
        let flat = Self::get_shopping_list(db, start_date, end_date).await?;
        let (staples, to_buy): (Vec<_>, Vec<_>) = flat.into_iter().partition(|item| {
            pantry_classifier::is_pantry_staple(
                &item.ingredient_name,
                item.total_unit.as_deref(),
                item.total_amount.as_ref(),
            )
        });
        Ok(ShoppingListSplitDto {
            items_to_buy: to_buy,
            pantry_staples_to_verify: staples,
        })
    }

    /// Builds the list [`Self::get_shopping_list`] returns for the same date
    /// range and rounds each line's total up to an amount a shopper can buy.
    ///
    /// Every item keeps the unrounded line, including its per-meal sources, in
    /// `aggregate`. The rounding rule comes from
    /// [`unit_converter::shopping_unit_class`] on the line's unit, and a range
    /// rounds each end and stays a range. A unitless line counts whole items,
    /// so it rounds up to the next whole one. An item has no shopping amount
    /// only when its sources could not be summed into one total, which the
    /// line reports as a `total_amount` of `None`.
    pub async fn get_shopping_list_rounded(
        db: &DatabaseConnection,
        start_date: String,
        end_date: String,
    ) -> Result<Vec<ShoppingListItemDto>, DbErr> {
        let list = Self::get_shopping_list(db, start_date, end_date).await?;
        Ok(list.into_iter().map(shopping_item_from_aggregate).collect())
    }
}

/// Wraps an aggregated line with its total rounded up for shopping.
fn shopping_item_from_aggregate(aggregate: AggregatedIngredientDto) -> ShoppingListItemDto {
    let shopping = match (&aggregate.total_amount, &aggregate.total_unit) {
        // Rounding starts from the total the aggregator already rounded to two
        // decimals for display, so noise below 0.005, as in 3.004 lemons, never
        // buys an extra step. A true total below 0.005 therefore arrives as 0
        // and passes through as 0 instead of rounding up to one step.
        (Some(amount), Some(unit)) => Some(round_amount_for_shopping(amount, unit)),
        // The aggregator sets the total and the unit together or sets neither,
        // so reaching this arm means the sources did not sum. A unitless line
        // is not one of those: its unit is an empty string, which classifies
        // as a count.
        _ => None,
    };
    ShoppingListItemDto {
        aggregate,
        shopping,
    }
}

fn round_amount_for_shopping(amount: &IngredientAmountDto, unit: &str) -> ShoppingAmountDto {
    let class = unit_converter::shopping_unit_class(unit);
    let (amount, rounded) = match *amount {
        IngredientAmountDto::Single { value } => {
            let (value, rounded) = unit_converter::round_up_for_shopping(value, class);
            (IngredientAmountDto::Single { value }, rounded)
        }
        IngredientAmountDto::Range { min, max } => {
            // Rounding never decreases as the input grows, so min stays at or
            // below max. The result stays a Range even when both ends meet.
            let (min, min_rounded) = unit_converter::round_up_for_shopping(min, class);
            let (max, max_rounded) = unit_converter::round_up_for_shopping(max, class);
            (
                IngredientAmountDto::Range { min, max },
                min_rounded || max_rounded,
            )
        }
    };
    ShoppingAmountDto {
        amount,
        class,
        rounded,
    }
}

/// Scale an ingredient amount by a factor
fn scale_amount(amount: &IngredientAmountDto, scale: f64) -> IngredientAmountDto {
    match amount {
        IngredientAmountDto::Single { value } => IngredientAmountDto::Single {
            value: value * scale,
        },
        IngredientAmountDto::Range { min, max } => IngredientAmountDto::Range {
            min: min * scale,
            max: max * scale,
        },
    }
}

/// Recursively scale an `or_alternative` ingredient (and any chained
/// alternatives) using the shopping pipeline's unrounded `scale_amount`.
/// Kept private here rather than reusing `recipe_scaler::scale_one` so the
/// shopping aggregator stays in full precision — `recipe_scaler` rounds to
/// 2 decimals for UI display, which would compound across alts.
fn scale_alt(alt: IngredientDto, scale: f64) -> IngredientDto {
    IngredientDto {
        amount: scale_amount(&alt.amount, scale),
        or_alternative: alt
            .or_alternative
            .map(|nested| Box::new(scale_alt(*nested, scale))),
        ..alt
    }
}

/// Merge sources that share the same meal + recipe + ingredient into a single line.
/// E.g., two people eating the same recipe in one meal → one combined source row.
fn merge_same_meal_sources(items: Vec<IngredientWithSource>) -> Vec<IngredientWithSource> {
    // Groups keep first-seen order, so the output follows meal order: the
    // display name takes the first meal's casing and sums run in one order.
    // Items in different units stay apart, so the aggregation can convert them.
    // Key: (ingredient_name_lower, meal_id, source_name, normalized_unit)
    let mut index: HashMap<(String, String, Option<String>, String), usize> = HashMap::new();
    let mut groups: Vec<Vec<IngredientWithSource>> = Vec::new();

    for item in items {
        let key = (
            item.ingredient.name.to_lowercase(),
            item.source.meal_id.clone(),
            item.source.source_name.clone(),
            unit_converter::normalize_unit(&item.ingredient.unit),
        );
        let slot = *index.entry(key).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[slot].push(item);
    }

    let mut merged = Vec::new();
    for group in groups {
        if group.len() <= 1 {
            merged.extend(group);
            continue;
        }

        // Check all amounts are the same variant (all Single or all Range)
        let all_single = group
            .iter()
            .all(|i| matches!(i.ingredient.amount, IngredientAmountDto::Single { .. }));
        let all_range = group
            .iter()
            .all(|i| matches!(i.ingredient.amount, IngredientAmountDto::Range { .. }));

        if !all_single && !all_range {
            // Mixed single/range from same recipe shouldn't happen; keep separate
            merged.extend(group);
            continue;
        }

        let mut total_person_servings: f64 = 0.0;
        let merged_amount = if all_single {
            let total: f64 = group
                .iter()
                .filter_map(|i| match &i.ingredient.amount {
                    IngredientAmountDto::Single { value } => Some(value),
                    _ => None,
                })
                .sum();
            IngredientAmountDto::Single { value: total }
        } else {
            let min_total: f64 = group
                .iter()
                .filter_map(|i| match &i.ingredient.amount {
                    IngredientAmountDto::Range { min, .. } => Some(min),
                    _ => None,
                })
                .sum();
            let max_total: f64 = group
                .iter()
                .filter_map(|i| match &i.ingredient.amount {
                    IngredientAmountDto::Range { max, .. } => Some(max),
                    _ => None,
                })
                .sum();
            IngredientAmountDto::Range {
                min: min_total,
                max: max_total,
            }
        };

        for item in &group {
            if let Some(ps) = item.source.person_servings {
                total_person_servings += ps;
            }
        }

        let mut base = group.into_iter().next().unwrap();
        base.ingredient.amount = merged_amount.clone();
        base.source.amount = merged_amount;
        base.source.person_servings = if base.source.recipe_servings.is_some() {
            Some(total_person_servings)
        } else {
            None
        };
        merged.push(base);
    }

    merged
}

/// Aggregate a group of same-name ingredients into one AggregatedIngredientDto
fn aggregate_group(items: Vec<IngredientWithSource>) -> AggregatedIngredientDto {
    // Use the display name from the first item (preserves original casing)
    let ingredient_name = items
        .first()
        .map(|i| i.ingredient.name.clone())
        .unwrap_or_default();

    let sources: Vec<IngredientSourceDto> = items.iter().map(|i| i.source.clone()).collect();

    // Try to sum amounts if units are compatible
    let (total_amount, total_unit) = try_sum_amounts(&items);

    AggregatedIngredientDto {
        ingredient_name,
        total_amount,
        total_unit,
        items: sources,
    }
}

/// Attempt to sum ingredient amounts. Returns (total_amount, total_unit)
/// if all items have compatible units, otherwise (None, None).
fn try_sum_amounts(
    items: &[IngredientWithSource],
) -> (Option<IngredientAmountDto>, Option<String>) {
    if items.is_empty() {
        return (None, None);
    }

    // Check if all items have the same unit category
    let categories: Vec<Option<&str>> = items
        .iter()
        .map(|i| unit_converter::unit_category(&i.ingredient.unit))
        .collect();

    let first_category = categories[0];

    // All must be the same category (and not None for discrete units)
    let all_same = categories.iter().all(|c| *c == first_category);

    if !all_same {
        return (None, None);
    }

    // Check if we have a mix of Single and Range amounts (MVP: don't sum mixed)
    let has_single = items
        .iter()
        .any(|i| matches!(i.ingredient.amount, IngredientAmountDto::Single { .. }));
    let has_range = items
        .iter()
        .any(|i| matches!(i.ingredient.amount, IngredientAmountDto::Range { .. }));

    if has_single && has_range {
        return (None, None);
    }

    match first_category {
        Some(category) => {
            // Convert all to base units, sum, then pick display unit
            if has_range {
                sum_ranges_with_conversion(items, category)
            } else {
                sum_singles_with_conversion(items, category)
            }
        }
        None => {
            // Discrete units — check if all are the same normalized unit
            let first_unit = unit_converter::normalize_unit(&items[0].ingredient.unit);
            let all_same_unit = items
                .iter()
                .all(|i| unit_converter::normalize_unit(&i.ingredient.unit) == first_unit);

            if !all_same_unit {
                return (None, None);
            }

            // Sum discrete amounts (same unit, no conversion needed)
            if has_range {
                sum_ranges_direct(items, &first_unit)
            } else {
                sum_singles_direct(items, &first_unit)
            }
        }
    }
}

/// Find the most common normalized unit among the items. A tie goes to the
/// larger unit, then to the alphabetically first one.
fn most_common_unit(items: &[IngredientWithSource]) -> String {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for item in items {
        let normalized = unit_converter::normalize_unit(&item.ingredient.unit);
        *counts.entry(normalized).or_default() += 1;
    }
    // The larger unit shows the smaller number. Within a category no two
    // units share a base factor, and callers only pass items of one category.
    // A unit with no factor ranks smallest.
    let factor = |unit: &str| unit_converter::to_base(1.0, unit).unwrap_or(0.0);
    counts
        .into_iter()
        .max_by(|(a_unit, a_count), (b_unit, b_count)| {
            a_count
                .cmp(b_count)
                .then_with(|| factor(a_unit).total_cmp(&factor(b_unit)))
                .then_with(|| b_unit.cmp(a_unit))
        })
        .map(|(unit, _)| unit)
        .unwrap_or_default()
}

fn sum_singles_with_conversion(
    items: &[IngredientWithSource],
    _category: &str,
) -> (Option<IngredientAmountDto>, Option<String>) {
    let mut base_total = 0.0;

    for item in items {
        if let IngredientAmountDto::Single { value } = &item.ingredient.amount {
            if let Some(base) = unit_converter::to_base(*value, &item.ingredient.unit) {
                base_total += base;
            } else {
                return (None, None);
            }
        }
    }

    // Display in the most common input unit for consistency
    let display_unit = most_common_unit(items);
    let display_value = unit_converter::from_base(base_total, &display_unit).unwrap_or(base_total);

    (
        Some(IngredientAmountDto::Single {
            value: round_display(display_value),
        }),
        Some(display_unit),
    )
}

fn sum_ranges_with_conversion(
    items: &[IngredientWithSource],
    _category: &str,
) -> (Option<IngredientAmountDto>, Option<String>) {
    let mut base_min_total = 0.0;
    let mut base_max_total = 0.0;

    for item in items {
        if let IngredientAmountDto::Range { min, max } = &item.ingredient.amount {
            match (
                unit_converter::to_base(*min, &item.ingredient.unit),
                unit_converter::to_base(*max, &item.ingredient.unit),
            ) {
                (Some(base_min), Some(base_max)) => {
                    base_min_total += base_min;
                    base_max_total += base_max;
                }
                _ => return (None, None),
            }
        }
    }

    // Display in the most common input unit for consistency
    let display_unit = most_common_unit(items);
    let display_min =
        unit_converter::from_base(base_min_total, &display_unit).unwrap_or(base_min_total);
    let display_max =
        unit_converter::from_base(base_max_total, &display_unit).unwrap_or(base_max_total);

    (
        Some(IngredientAmountDto::Range {
            min: round_display(display_min),
            max: round_display(display_max),
        }),
        Some(display_unit),
    )
}

fn sum_singles_direct(
    items: &[IngredientWithSource],
    unit: &str,
) -> (Option<IngredientAmountDto>, Option<String>) {
    let mut total = 0.0;
    for item in items {
        if let IngredientAmountDto::Single { value } = &item.ingredient.amount {
            total += value;
        }
    }
    (
        Some(IngredientAmountDto::Single {
            value: round_display(total),
        }),
        Some(unit.to_string()),
    )
}

fn sum_ranges_direct(
    items: &[IngredientWithSource],
    unit: &str,
) -> (Option<IngredientAmountDto>, Option<String>) {
    let mut min_total = 0.0;
    let mut max_total = 0.0;
    for item in items {
        if let IngredientAmountDto::Range { min, max } = &item.ingredient.amount {
            min_total += min;
            max_total += max;
        }
    }
    (
        Some(IngredientAmountDto::Range {
            min: round_display(min_total),
            max: round_display(max_total),
        }),
        Some(unit.to_string()),
    )
}

/// Round to 2 decimal places for display
fn round_display(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::unit_converter::ShoppingUnitClass;

    fn source(amount: IngredientAmountDto, unit: &str, recipe: &str) -> IngredientSourceDto {
        IngredientSourceDto {
            amount,
            unit: unit.to_string(),
            source_type: SourceType::Recipe,
            source_name: Some(recipe.to_string()),
            meal_id: format!("meal-{recipe}"),
            meal_date: "2025-06-10".to_string(),
            meal_type: "Dinner".to_string(),
            recipe_servings: Some(4),
            person_servings: Some(1.0),
        }
    }

    fn aggregate(
        total: Option<IngredientAmountDto>,
        unit: Option<&str>,
    ) -> AggregatedIngredientDto {
        let source_unit = unit.unwrap_or("lb");
        AggregatedIngredientDto {
            ingredient_name: "Potatoes".to_string(),
            total_amount: total,
            total_unit: unit.map(str::to_string),
            items: vec![
                source(
                    IngredientAmountDto::Single { value: 1.0 },
                    source_unit,
                    "Hash",
                ),
                source(
                    IngredientAmountDto::Range {
                        min: 0.5,
                        max: 0.75,
                    },
                    source_unit,
                    "Stew",
                ),
            ],
        }
    }

    // The DTOs do not derive PartialEq, so the aggregate is compared as JSON.
    fn assert_aggregate_unchanged(item: &ShoppingListItemDto, input: &AggregatedIngredientDto) {
        assert_eq!(
            serde_json::to_value(&item.aggregate).unwrap(),
            serde_json::to_value(input).unwrap()
        );
    }

    fn rounded_item(
        total: IngredientAmountDto,
        unit: &str,
    ) -> (ShoppingListItemDto, AggregatedIngredientDto) {
        let input = aggregate(Some(total), Some(unit));
        let item = shopping_item_from_aggregate(input.clone());
        assert_aggregate_unchanged(&item, &input);
        (item, input)
    }

    #[test]
    fn single_weight_rounds_up_and_keeps_the_exact_total_and_sources() {
        let (item, _) = rounded_item(IngredientAmountDto::Single { value: 1.75 }, "lb");
        let shopping = item.shopping.unwrap();
        assert!(matches!(shopping.amount, IngredientAmountDto::Single { value } if value == 2.0));
        assert_eq!(item.aggregate.total_unit.as_deref(), Some("lb"));
        assert!(matches!(shopping.class, ShoppingUnitClass::Graduated));
        assert!(shopping.rounded);
        assert!(matches!(
            item.aggregate.total_amount,
            Some(IngredientAmountDto::Single { value }) if value == 1.75
        ));
    }

    #[test]
    fn unitless_range_stays_a_range_when_both_ends_meet() {
        let (item, _) = rounded_item(IngredientAmountDto::Range { min: 0.8, max: 0.9 }, "");
        let shopping = item.shopping.unwrap();
        assert!(matches!(
            shopping.amount,
            IngredientAmountDto::Range { min, max } if min == 1.0 && max == 1.0
        ));
        assert!(matches!(shopping.class, ShoppingUnitClass::Count));
        assert!(shopping.rounded);
    }

    #[test]
    fn graduated_range_rounds_each_end_by_its_own_tier() {
        let (item, _) = rounded_item(IngredientAmountDto::Range { min: 1.8, max: 2.2 }, "cup");
        let shopping = item.shopping.unwrap();
        assert!(matches!(
            shopping.amount,
            IngredientAmountDto::Range { min, max } if min == 2.0 && max == 3.0
        ));
        assert!(shopping.rounded);
    }

    #[test]
    fn range_already_on_buyable_values_is_not_rounded() {
        let (item, _) = rounded_item(IngredientAmountDto::Range { min: 1.0, max: 2.0 }, "cup");
        let shopping = item.shopping.unwrap();
        assert!(matches!(
            shopping.amount,
            IngredientAmountDto::Range { min, max } if min == 1.0 && max == 2.0
        ));
        assert!(!shopping.rounded);
    }

    #[test]
    fn small_measurement_total_passes_through() {
        let (item, _) = rounded_item(IngredientAmountDto::Single { value: 0.875 }, "tsp");
        let shopping = item.shopping.unwrap();
        assert!(matches!(shopping.amount, IngredientAmountDto::Single { value } if value == 0.875));
        assert!(matches!(shopping.class, ShoppingUnitClass::PassThrough));
        assert!(!shopping.rounded);
    }

    #[test]
    fn to_taste_total_with_an_amount_passes_through() {
        let (item, _) = rounded_item(IngredientAmountDto::Single { value: 2.0 }, "to taste");
        let shopping = item.shopping.unwrap();
        assert!(matches!(shopping.amount, IngredientAmountDto::Single { value } if value == 2.0));
        assert!(matches!(shopping.class, ShoppingUnitClass::PassThrough));
        assert!(!shopping.rounded);
    }

    #[test]
    fn line_without_a_total_or_unit_has_no_shopping_amount() {
        let cases = [
            aggregate(None, None),
            aggregate(None, Some("lb")),
            aggregate(Some(IngredientAmountDto::Single { value: 1.75 }), None),
        ];
        for input in cases {
            let item = shopping_item_from_aggregate(input.clone());
            assert!(item.shopping.is_none());
            assert_aggregate_unchanged(&item, &input);
        }
    }

    fn item(name: &str, value: f64, unit: &str) -> IngredientWithSource {
        let amount = IngredientAmountDto::Single { value };
        IngredientWithSource {
            ingredient: IngredientDto {
                name: name.to_string(),
                prep: None,
                amount: amount.clone(),
                unit: unit.to_string(),
                notes: None,
                or_alternative: None,
            },
            source: source(amount, unit, name),
        }
    }

    // Every call builds a fresh HashMap with its own random hash keys, so a
    // tie broken by iteration order picks either unit about half the time.
    // Repeating each order 64 times makes a lucky pass vanishingly unlikely.
    #[test]
    fn most_common_unit_tie_prefers_the_larger_unit() {
        for _ in 0..64 {
            let forward = [item("flour", 1.0, "cup"), item("flour", 1.0, "tbsp")];
            let reverse = [item("flour", 1.0, "tbsp"), item("flour", 1.0, "cup")];
            assert_eq!(most_common_unit(&forward), "cup");
            assert_eq!(most_common_unit(&reverse), "cup");
        }
        let three_way = [
            item("flour", 1.0, "cup"),
            item("flour", 1.0, "tbsp"),
            item("flour", 1.0, "tsp"),
        ];
        assert_eq!(most_common_unit(&three_way), "cup");
    }

    #[test]
    fn most_common_unit_count_beats_size() {
        let items = [
            item("flour", 2.0, "tbsp"),
            item("flour", 1.0, "cup"),
            item("flour", 1.0, "tbsp"),
        ];
        assert_eq!(most_common_unit(&items), "tbsp");
    }

    #[test]
    fn tied_units_total_in_the_larger_unit() {
        for _ in 0..64 {
            let items = [item("flour", 1.0, "tbsp"), item("flour", 1.0, "cup")];
            let (total, unit) = sum_singles_with_conversion(&items, "volume");
            assert_eq!(unit.as_deref(), Some("cup"));
            assert!(
                matches!(total, Some(IngredientAmountDto::Single { value }) if value == 1.06),
                "{total:?}"
            );
        }
    }

    // Two sources from different meals form two merge groups, and their
    // output order must follow the input, which is meal order.
    #[test]
    fn merge_same_meal_sources_keeps_input_order() {
        for _ in 0..64 {
            let mut first = item("Flour", 2.0, "cup");
            first.source.meal_id = "meal-a".to_string();
            let mut second = item("flour", 1.0, "tbsp");
            second.source.meal_id = "meal-b".to_string();
            let merged = merge_same_meal_sources(vec![first, second]);
            let meals: Vec<&str> = merged.iter().map(|i| i.source.meal_id.as_str()).collect();
            assert_eq!(meals, ["meal-a", "meal-b"]);
            assert_eq!(merged[0].ingredient.name, "Flour");
        }
    }
}
