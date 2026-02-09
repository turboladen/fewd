use std::collections::{HashMap, HashSet};

use chrono::{NaiveDate, Utc};
use sea_orm::*;
use serde::{Deserialize, Serialize};

use crate::commands::meal::PersonServingDto;
use crate::entities::meal::{self, Entity as Meal};
use crate::entities::recipe::{self, Entity as Recipe};

const RECENT_DAYS: i64 = 14;
const FORGOTTEN_MIN_TIMES: i32 = 3;
const FORGOTTEN_MIN_RATING: f64 = 4.0;
const FORGOTTEN_DAYS_SINCE: i64 = 30;
const MAX_PER_CATEGORY: usize = 5;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SuggestionItem {
    pub recipe_id: String,
    pub recipe_name: String,
    pub rating: Option<f64>,
    pub last_made: Option<chrono::DateTime<Utc>>,
    pub times_made: i32,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MealSuggestions {
    pub recent_favorites: Vec<SuggestionItem>,
    pub forgotten_hits: Vec<SuggestionItem>,
    pub untried: Vec<SuggestionItem>,
}

pub struct SuggestionService;

impl SuggestionService {
    pub async fn get_suggestions(
        db: &DatabaseConnection,
        person_ids: Vec<String>,
        reference_date: NaiveDate,
    ) -> Result<MealSuggestions, DbErr> {
        let person_set: HashSet<String> = person_ids.into_iter().collect();

        let recent_favorites = Self::recent_favorites(db, &person_set, reference_date).await?;
        let forgotten_hits = Self::forgotten_hits(db, reference_date).await?;
        let untried = Self::untried(db, &person_set, &recent_favorites, &forgotten_hits).await?;

        Ok(MealSuggestions {
            recent_favorites,
            forgotten_hits,
            untried,
        })
    }

    /// Recipes most used in the past 14 days for the selected people
    async fn recent_favorites(
        db: &DatabaseConnection,
        person_set: &HashSet<String>,
        reference_date: NaiveDate,
    ) -> Result<Vec<SuggestionItem>, DbErr> {
        let start_date = reference_date - chrono::Duration::days(RECENT_DAYS);

        let meals = Meal::find()
            .filter(meal::Column::Date.gte(start_date))
            .filter(meal::Column::Date.lte(reference_date))
            .all(db)
            .await?;

        // Count recipe usage per recipe_id for the selected people
        let mut recipe_counts: HashMap<String, u32> = HashMap::new();
        for meal in &meals {
            let servings = parse_servings(&meal.servings)?;
            for serving in &servings {
                if let PersonServingDto::Recipe {
                    person_id,
                    recipe_id,
                    ..
                } = serving
                {
                    if person_set.contains(person_id) {
                        *recipe_counts.entry(recipe_id.clone()).or_insert(0) += 1;
                    }
                }
            }
        }

        // Sort by count descending
        let mut sorted: Vec<(String, u32)> = recipe_counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(MAX_PER_CATEGORY);

        // Look up recipe details
        let mut results = Vec::new();
        for (recipe_id, count) in sorted {
            if let Some(recipe) = Recipe::find_by_id(&recipe_id).one(db).await? {
                results.push(SuggestionItem {
                    recipe_id: recipe.id,
                    recipe_name: recipe.name,
                    rating: recipe.rating,
                    last_made: recipe.last_made,
                    times_made: recipe.times_made,
                    reason: format!(
                        "Made {} time{} in the last 2 weeks",
                        count,
                        if count == 1 { "" } else { "s" }
                    ),
                });
            }
        }

        Ok(results)
    }

    /// Well-rated recipes with high usage that haven't been made recently
    async fn forgotten_hits(
        db: &DatabaseConnection,
        reference_date: NaiveDate,
    ) -> Result<Vec<SuggestionItem>, DbErr> {
        let cutoff = reference_date - chrono::Duration::days(FORGOTTEN_DAYS_SINCE);
        let cutoff_dt = cutoff.and_hms_opt(0, 0, 0).unwrap().and_utc();

        // Recipes with enough usage and good rating
        let candidates = Recipe::find()
            .filter(recipe::Column::TimesMade.gte(FORGOTTEN_MIN_TIMES))
            .filter(recipe::Column::Rating.gte(FORGOTTEN_MIN_RATING))
            .all(db)
            .await?;

        // Filter to those not made recently
        let mut hits: Vec<&recipe::Model> = candidates
            .iter()
            .filter(|r| match r.last_made {
                None => true,
                Some(last) => last < cutoff_dt,
            })
            .collect();

        // Sort by rating desc, then times_made desc
        hits.sort_by(|a, b| {
            b.rating
                .unwrap_or(0.0)
                .partial_cmp(&a.rating.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.times_made.cmp(&a.times_made))
        });
        hits.truncate(MAX_PER_CATEGORY);

        let results = hits
            .into_iter()
            .map(|recipe| {
                let days_ago = match recipe.last_made {
                    Some(last) => {
                        let days = (Utc::now() - last).num_days();
                        format!("last made {} days ago", days)
                    }
                    None => "never recorded".to_string(),
                };
                SuggestionItem {
                    recipe_id: recipe.id.clone(),
                    recipe_name: recipe.name.clone(),
                    rating: recipe.rating,
                    last_made: recipe.last_made,
                    times_made: recipe.times_made,
                    reason: format!(
                        "Rated {}★, made {} times, {}",
                        recipe.rating.unwrap_or(0.0) as i32,
                        recipe.times_made,
                        days_ago
                    ),
                }
            })
            .collect();

        Ok(results)
    }

    /// Recipes never assigned to any of the selected people
    async fn untried(
        db: &DatabaseConnection,
        person_set: &HashSet<String>,
        recent: &[SuggestionItem],
        forgotten: &[SuggestionItem],
    ) -> Result<Vec<SuggestionItem>, DbErr> {
        // Fetch all meals to find every recipe each selected person has had
        let all_meals = Meal::find().all(db).await?;

        let mut tried_recipe_ids: HashSet<String> = HashSet::new();
        for meal in &all_meals {
            let servings = parse_servings(&meal.servings)?;
            for serving in &servings {
                if let PersonServingDto::Recipe {
                    person_id,
                    recipe_id,
                    ..
                } = serving
                {
                    if person_set.contains(person_id) {
                        tried_recipe_ids.insert(recipe_id.clone());
                    }
                }
            }
        }

        // Also exclude recipes already in other suggestion categories
        let already_suggested: HashSet<String> = recent
            .iter()
            .chain(forgotten.iter())
            .map(|s| s.recipe_id.clone())
            .collect();

        // Get all recipes not in tried set and not already suggested
        let all_recipes = Recipe::find()
            .order_by_asc(recipe::Column::Name)
            .all(db)
            .await?;

        let mut candidates: Vec<&recipe::Model> = all_recipes
            .iter()
            .filter(|r| !tried_recipe_ids.contains(&r.id) && !already_suggested.contains(&r.id))
            .collect();

        // Sort by rating desc (nulls last), then name
        candidates.sort_by(|a, b| {
            let ra = a.rating.unwrap_or(-1.0);
            let rb = b.rating.unwrap_or(-1.0);
            rb.partial_cmp(&ra)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.name.cmp(&b.name))
        });
        candidates.truncate(MAX_PER_CATEGORY);

        let results = candidates
            .into_iter()
            .map(|recipe| SuggestionItem {
                recipe_id: recipe.id.clone(),
                recipe_name: recipe.name.clone(),
                rating: recipe.rating,
                last_made: recipe.last_made,
                times_made: recipe.times_made,
                reason: "Never tried by the selected people".to_string(),
            })
            .collect();

        Ok(results)
    }
}

fn parse_servings(json: &str) -> Result<Vec<PersonServingDto>, DbErr> {
    serde_json::from_str(json)
        .map_err(|e| DbErr::Custom(format!("Failed to parse servings: {}", e)))
}
