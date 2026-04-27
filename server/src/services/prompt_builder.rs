use std::collections::HashMap;
use std::fmt::Write;

use chrono::Datelike;

use crate::dto::PersonServingDto;
use crate::entities::{meal, person, recipe};

const MAX_HISTORY_ENTRIES: usize = 30;

pub struct PromptBuilder;

impl PromptBuilder {
    /// Format person profiles for inclusion in a prompt
    pub fn build_person_context(people: &[person::Model]) -> String {
        let mut output = String::new();

        for person in people {
            let age = compute_age(person.birthdate);
            let _ = writeln!(output, "## {} (age {})", person.name, age);

            if let Some(ref goals) = person.dietary_goals {
                if !goals.is_empty() {
                    let _ = writeln!(output, "- Dietary goals: {}", goals);
                }
            }

            let dislikes = parse_string_array(&person.dislikes);
            if !dislikes.is_empty() {
                let _ = writeln!(output, "- Dislikes: {}", dislikes.join(", "));
            }

            let favorites = parse_string_array(&person.favorites);
            if !favorites.is_empty() {
                let _ = writeln!(output, "- Favorites: {}", favorites.join(", "));
            }

            if let Some(ref notes) = person.notes {
                if !notes.is_empty() {
                    let _ = writeln!(output, "- Notes: {}", notes);
                }
            }

            output.push('\n');
        }

        output.trim_end().to_string()
    }

    /// Format a recipe for inclusion in a prompt
    pub fn build_recipe_context(recipe: &recipe::Model) -> String {
        let mut output = String::new();

        let _ = writeln!(output, "## {} (serves {})", recipe.name, recipe.servings);

        if let Some(ref desc) = recipe.description {
            if !desc.is_empty() {
                let _ = writeln!(output, "{}", desc);
            }
        }

        let _ = writeln!(output);

        // Ingredients
        let ingredients = parse_ingredients(&recipe.ingredients);
        if !ingredients.is_empty() {
            let _ = writeln!(output, "Ingredients:");
            for ing in &ingredients {
                let _ = writeln!(output, "- {}", ing);
            }
            let _ = writeln!(output);
        }

        // Instructions
        if !recipe.instructions.is_empty() {
            let _ = writeln!(output, "Instructions:");
            let _ = writeln!(output, "{}", recipe.instructions);
            let _ = writeln!(output);
        }

        // Tags
        let tags = parse_string_array(&recipe.tags);
        if !tags.is_empty() {
            let _ = writeln!(output, "Tags: {}", tags.join(", "));
        }

        // Nutrition
        if let Some(ref nutrition) = recipe.nutrition_per_serving {
            let nutrition_str = format_nutrition(nutrition);
            if !nutrition_str.is_empty() {
                let _ = writeln!(output, "Nutrition per serving: {}", nutrition_str);
            }
        }

        if let Some(ref notes) = recipe.notes {
            if !notes.is_empty() {
                let _ = writeln!(output, "Notes: {}", notes);
            }
        }

        output.trim_end().to_string()
    }

    /// Format recent meal history as context
    pub fn build_meal_history_context(meals: &[meal::Model], recipes: &[recipe::Model]) -> String {
        if meals.is_empty() {
            return "No recent meal history.".to_string();
        }

        // Build recipe name lookup
        let recipe_map: HashMap<&str, &str> = recipes
            .iter()
            .map(|r| (r.id.as_str(), r.name.as_str()))
            .collect();

        let mut output = String::from("Recent meals:\n");

        let entries = if meals.len() > MAX_HISTORY_ENTRIES {
            &meals[..MAX_HISTORY_ENTRIES]
        } else {
            meals
        };

        for meal in entries {
            let servings = parse_servings(&meal.servings);
            if servings.is_empty() {
                continue;
            }

            let items: Vec<String> = servings
                .iter()
                .map(|s| match s {
                    PersonServingDto::Recipe {
                        recipe_id,
                        servings_count,
                        ..
                    } => {
                        let name = recipe_map
                            .get(recipe_id.as_str())
                            .unwrap_or(&"Unknown recipe");
                        if (*servings_count - 1.0).abs() < f64::EPSILON {
                            name.to_string()
                        } else {
                            format!("{} ({} servings)", name, servings_count)
                        }
                    }
                    PersonServingDto::Adhoc { adhoc_items, .. } => {
                        let names: Vec<&str> =
                            adhoc_items.iter().map(|i| i.name.as_str()).collect();
                        if names.is_empty() {
                            "adhoc items".to_string()
                        } else {
                            names.join(", ")
                        }
                    }
                })
                .collect();

            let _ = writeln!(
                output,
                "- {} {}: {}",
                meal.date,
                meal.meal_type,
                items.join("; ")
            );
        }

        if meals.len() > MAX_HISTORY_ENTRIES {
            let _ = writeln!(
                output,
                "... and {} more meals",
                meals.len() - MAX_HISTORY_ENTRIES
            );
        }

        output.trim_end().to_string()
    }
}

fn compute_age(birthdate: chrono::NaiveDate) -> i32 {
    let today = chrono::Utc::now().date_naive();
    let mut age = today.year() - birthdate.year();
    if (today.month(), today.day()) < (birthdate.month(), birthdate.day()) {
        age -= 1;
    }
    age
}

fn parse_string_array(json: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json).unwrap_or_default()
}

fn parse_servings(json: &str) -> Vec<PersonServingDto> {
    serde_json::from_str(json).unwrap_or_default()
}

/// Parse ingredients JSON and format as readable strings
fn parse_ingredients(json: &str) -> Vec<String> {
    #[derive(serde::Deserialize)]
    struct Ingredient {
        name: String,
        #[serde(default)]
        prep: Option<String>,
        amount: serde_json::Value,
        unit: String,
        #[serde(default)]
        notes: Option<String>,
    }

    let ingredients: Vec<Ingredient> = serde_json::from_str(json).unwrap_or_default();

    ingredients
        .into_iter()
        .map(|ing| {
            let amount_str = match ing.amount.get("type").and_then(|t| t.as_str()) {
                Some("single") => ing
                    .amount
                    .get("value")
                    .and_then(|v| v.as_f64())
                    .map(format_amount)
                    .unwrap_or_default(),
                Some("range") => {
                    let min = ing
                        .amount
                        .get("min")
                        .and_then(|v| v.as_f64())
                        .map(format_amount)
                        .unwrap_or_default();
                    let max = ing
                        .amount
                        .get("max")
                        .and_then(|v| v.as_f64())
                        .map(format_amount)
                        .unwrap_or_default();
                    format!("{}-{}", min, max)
                }
                _ => String::new(),
            };

            let label = match ing.prep.as_deref() {
                Some(prep) if !prep.is_empty() => format!("{}, {}", ing.name, prep),
                _ => ing.name.clone(),
            };

            let mut result = if amount_str.is_empty() {
                label
            } else if ing.unit.is_empty() {
                format!("{} {}", amount_str, label)
            } else {
                format!("{} {} {}", amount_str, ing.unit, label)
            };

            if let Some(notes) = ing.notes {
                if !notes.is_empty() {
                    result.push_str(&format!(" ({})", notes));
                }
            }

            result
        })
        .collect()
}

fn format_amount(val: f64) -> String {
    if (val - val.round()).abs() < f64::EPSILON {
        format!("{}", val as i64)
    } else {
        format!("{:.1}", val)
    }
}

/// Parse nutrition JSON and format as a readable string
fn format_nutrition(json: &str) -> String {
    #[derive(serde::Deserialize)]
    struct Nutrition {
        calories: Option<f64>,
        protein_grams: Option<f64>,
        carbs_grams: Option<f64>,
        fat_grams: Option<f64>,
        notes: Option<String>,
    }

    let nutrition: Nutrition = match serde_json::from_str(json) {
        Ok(n) => n,
        Err(_) => return String::new(),
    };

    let mut parts = Vec::new();
    if let Some(cal) = nutrition.calories {
        parts.push(format!("{} cal", cal as i64));
    }
    if let Some(protein) = nutrition.protein_grams {
        parts.push(format!("{}g protein", protein as i64));
    }
    if let Some(carbs) = nutrition.carbs_grams {
        parts.push(format!("{}g carbs", carbs as i64));
    }
    if let Some(fat) = nutrition.fat_grams {
        parts.push(format!("{}g fat", fat as i64));
    }
    if let Some(ref notes) = nutrition.notes {
        if !notes.is_empty() {
            parts.push(notes.clone());
        }
    }

    parts.join(", ")
}
