use std::collections::HashMap;

use sea_orm::*;

use crate::dto::PersonServingDto;
use crate::dto::{IngredientAmountDto, IngredientDto};
use crate::dto::{AggregatedIngredientDto, IngredientSourceDto, SourceType};
use crate::entities::recipe::Entity as Recipe;
use crate::services::meal_service::MealService;
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
                        let recipe = Recipe::find_by_id(recipe_id).one(db).await?;

                        if let Some(recipe) = recipe {
                            let ingredients: Vec<IngredientDto> =
                                serde_json::from_str(&recipe.ingredients).map_err(|e| {
                                    DbErr::Custom(format!(
                                        "Failed to parse ingredients for recipe {}: {}",
                                        recipe.id, e
                                    ))
                                })?;

                            let scale = servings_count / recipe.servings as f64;

                            for ing in ingredients {
                                let scaled_amount = scale_amount(&ing.amount, scale);
                                let scaled_ingredient = IngredientDto {
                                    name: ing.name,
                                    amount: scaled_amount.clone(),
                                    unit: ing.unit.clone(),
                                    notes: ing.notes,
                                };

                                let source = IngredientSourceDto {
                                    amount: scaled_amount,
                                    unit: ing.unit,
                                    source_type: SourceType::Recipe,
                                    source_name: Some(recipe.name.clone()),
                                    meal_id: meal.id.clone(),
                                    meal_date: meal_date.clone(),
                                    meal_type: meal.meal_type.clone(),
                                    recipe_servings: Some(recipe.servings),
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
                                meal_type: meal.meal_type.clone(),
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

/// Merge sources that share the same meal + recipe + ingredient into a single line.
/// E.g., two people eating the same recipe in one meal → one combined source row.
fn merge_same_meal_sources(items: Vec<IngredientWithSource>) -> Vec<IngredientWithSource> {
    // Key: (ingredient_name_lower, meal_id, source_name)
    let mut groups: HashMap<(String, String, Option<String>), Vec<IngredientWithSource>> =
        HashMap::new();

    for item in items {
        let key = (
            item.ingredient.name.to_lowercase(),
            item.source.meal_id.clone(),
            item.source.source_name.clone(),
        );
        groups.entry(key).or_default().push(item);
    }

    let mut merged = Vec::new();
    for (_, group) in groups {
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

/// Find the most common normalized unit among the items.
fn most_common_unit(items: &[IngredientWithSource]) -> String {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for item in items {
        let normalized = unit_converter::normalize_unit(&item.ingredient.unit);
        *counts.entry(normalized).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
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
