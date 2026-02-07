use chrono::NaiveDate;
use sea_orm::*;

use crate::commands::meal::{CreateMealDto, PersonServingDto, UpdateMealDto};
use crate::entities::meal::{self, Entity as Meal};
use crate::entities::recipe::{self, Entity as Recipe};

pub struct MealService;

impl MealService {
    pub async fn get_all_for_date_range(
        db: &DatabaseConnection,
        start_date: String,
        end_date: String,
    ) -> Result<Vec<meal::Model>, DbErr> {
        let start = NaiveDate::parse_from_str(&start_date, "%Y-%m-%d")
            .map_err(|e| DbErr::Custom(format!("Invalid start_date format: {}", e)))?;
        let end = NaiveDate::parse_from_str(&end_date, "%Y-%m-%d")
            .map_err(|e| DbErr::Custom(format!("Invalid end_date format: {}", e)))?;

        Meal::find()
            .filter(meal::Column::Date.gte(start))
            .filter(meal::Column::Date.lte(end))
            .order_by_asc(meal::Column::Date)
            .order_by_asc(meal::Column::OrderIndex)
            .all(db)
            .await
    }

    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: String,
    ) -> Result<Option<meal::Model>, DbErr> {
        Meal::find_by_id(id).one(db).await
    }

    pub async fn create(
        db: &DatabaseConnection,
        data: CreateMealDto,
    ) -> Result<meal::Model, DbErr> {
        let now = chrono::Utc::now();
        let date = NaiveDate::parse_from_str(&data.date, "%Y-%m-%d")
            .map_err(|e| DbErr::Custom(format!("Invalid date format: {}", e)))?;

        let meal = meal::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            date: Set(date),
            meal_type: Set(data.meal_type),
            order_index: Set(data.order_index),
            servings: Set(serde_json::to_string(&data.servings).unwrap()),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let result = meal.insert(db).await?;

        // Increment times_made for any recipes used in this meal
        increment_recipe_usage(db, &data.servings).await?;

        Ok(result)
    }

    pub async fn update(
        db: &DatabaseConnection,
        id: String,
        data: UpdateMealDto,
    ) -> Result<meal::Model, DbErr> {
        let existing = Meal::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Meal not found".to_string()))?;

        let mut meal: meal::ActiveModel = existing.into();

        if let Some(date) = data.date {
            let parsed = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                .map_err(|e| DbErr::Custom(format!("Invalid date format: {}", e)))?;
            meal.date = Set(parsed);
        }
        if let Some(meal_type) = data.meal_type {
            meal.meal_type = Set(meal_type);
        }
        if let Some(order_index) = data.order_index {
            meal.order_index = Set(order_index);
        }
        if let Some(ref servings) = data.servings {
            meal.servings = Set(serde_json::to_string(servings).unwrap());
        }

        meal.updated_at = Set(chrono::Utc::now());

        meal.update(db).await
    }

    pub async fn delete(db: &DatabaseConnection, id: String) -> Result<(), DbErr> {
        Meal::delete_by_id(id).exec(db).await?;
        Ok(())
    }
}

/// Increment times_made and update last_made for each recipe referenced in servings
async fn increment_recipe_usage(
    db: &DatabaseConnection,
    servings: &[PersonServingDto],
) -> Result<(), DbErr> {
    let now = chrono::Utc::now();

    for serving in servings {
        if let PersonServingDto::Recipe { recipe_id, .. } = serving {
            if let Some(existing) = Recipe::find_by_id(recipe_id).one(db).await? {
                let new_times_made = existing.times_made + 1;
                let mut recipe: recipe::ActiveModel = existing.into();
                recipe.times_made = Set(new_times_made);
                recipe.last_made = Set(Some(now));
                recipe.updated_at = Set(now);
                recipe.update(db).await?;
            }
        }
    }

    Ok(())
}
