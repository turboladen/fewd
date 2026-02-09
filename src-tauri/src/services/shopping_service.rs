use std::collections::HashMap;

use sea_orm::*;

use crate::commands::meal::PersonServingDto;
use crate::commands::recipe::{IngredientAmountDto, IngredientDto};
use crate::commands::shopping::{AggregatedIngredientDto, IngredientSourceDto, SourceType};
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

        // 3. Group by ingredient name (case-insensitive)
        let mut groups: HashMap<String, Vec<IngredientWithSource>> = HashMap::new();
        for item in all_items {
            let key = item.ingredient.name.to_lowercase();
            groups.entry(key).or_default().push(item);
        }

        // 4. Aggregate each group
        let mut result: Vec<AggregatedIngredientDto> =
            groups.into_values().map(aggregate_group).collect();

        // 5. Sort alphabetically
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

fn sum_singles_with_conversion(
    items: &[IngredientWithSource],
    category: &str,
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

    let (display_value, display_unit) = unit_converter::best_display_unit(base_total, category);

    (
        Some(IngredientAmountDto::Single {
            value: round_display(display_value),
        }),
        Some(display_unit),
    )
}

fn sum_ranges_with_conversion(
    items: &[IngredientWithSource],
    category: &str,
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

    // Use max total to pick display unit (the larger value determines best unit)
    let (_, display_unit) = unit_converter::best_display_unit(base_max_total, category);

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
