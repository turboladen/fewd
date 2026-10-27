use std::collections::HashSet;

use chrono::NaiveDate;
use sea_orm::*;

use crate::dto::{CreateMealDto, PersonServingDto, UpdateMealDto};
use crate::entities::meal::{self, Entity as Meal};
use crate::services::normalize::trim_adhoc_item_names;
use crate::services::service_error::{check_servings_counts, ServiceError, ValidationError};
use crate::services::{begin_write, to_json};

pub struct MealService;

/// Parse a meal date sent as `YYYY-MM-DD`.
fn parse_meal_date(date: &str) -> Result<NaiveDate, ValidationError> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| ValidationError::InvalidDate(date.to_string()))
}

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
            // Two meals may share a date and slot, and the shopping list
            // takes its display names and source order from this order.
            .order_by_asc(meal::Column::CreatedAt)
            .order_by_asc(meal::Column::Id)
            .all(db)
            .await
    }

    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: String,
    ) -> Result<Option<meal::Model>, DbErr> {
        Meal::find_by_id(id).one(db).await
    }

    /// Creates a meal and counts one plan for each distinct recipe it serves.
    /// The meal row and the counters commit together or not at all.
    pub async fn create(
        db: &DatabaseConnection,
        mut data: CreateMealDto,
    ) -> Result<meal::Model, ServiceError> {
        check_servings_counts(&data.servings)?;
        trim_adhoc_item_names(&mut data.servings)?;
        let date = parse_meal_date(&data.date)?;
        let now = chrono::Utc::now();

        let meal = meal::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            date: Set(date),
            meal_type: Set(data.meal_type),
            order_index: Set(data.order_index),
            servings: Set(to_json(&data.servings)?),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let txn = begin_write(db).await?;
        let result = meal.insert(&txn).await?;
        let ids = collect_recipe_ids(&data.servings);
        increment_recipe_usage(&txn, ids.iter().map(String::as_str)).await?;
        txn.commit().await?;

        Ok(result)
    }

    /// Updates a meal. When the servings change, recipes that left the meal
    /// lose one plan and recipes that joined it gain one. When the stored
    /// servings cannot be read, no recipe's count changes and a warning
    /// names the meal. The meal row and the counters commit together or not
    /// at all.
    pub async fn update(
        db: &DatabaseConnection,
        id: String,
        mut data: UpdateMealDto,
    ) -> Result<meal::Model, ServiceError> {
        // Every check runs before the transaction opens.
        if let Some(servings) = &mut data.servings {
            check_servings_counts(servings)?;
            trim_adhoc_item_names(servings)?;
        }
        let date = data.date.as_deref().map(parse_meal_date).transpose()?;

        // Lock before reading, so the stored servings cannot change before the
        // counters move.
        let txn = begin_write(db).await?;
        let existing = Meal::find_by_id(id)
            .one(&txn)
            .await?
            .ok_or(DbErr::RecordNotFound("Meal not found".to_string()))?;

        // Counters move only when both the old and the new recipe sets are
        // known; otherwise a recipe in both could be counted twice.
        if let Some(new_servings) = &data.servings {
            if let Some(old_ids) = stored_recipe_ids(&existing) {
                let new_ids = collect_recipe_ids(new_servings);
                decrement_recipe_usage(&txn, old_ids.difference(&new_ids).map(String::as_str))
                    .await?;
                increment_recipe_usage(&txn, new_ids.difference(&old_ids).map(String::as_str))
                    .await?;
            }
        }

        let mut meal: meal::ActiveModel = existing.into();

        if let Some(date) = date {
            meal.date = Set(date);
        }
        if let Some(meal_type) = data.meal_type {
            meal.meal_type = Set(meal_type);
        }
        if let Some(order_index) = data.order_index {
            meal.order_index = Set(order_index);
        }
        if let Some(ref servings) = data.servings {
            meal.servings = Set(to_json(servings)?);
        }

        meal.updated_at = Set(chrono::Utc::now());

        let updated = meal.update(&txn).await?;
        txn.commit().await?;
        Ok(updated)
    }

    /// Deletes a meal and removes one plan from each distinct recipe it
    /// served. When the stored servings cannot be read, no recipe's count
    /// changes and a warning names the meal. The deletion and the counters
    /// commit together or not at all.
    pub async fn delete(db: &DatabaseConnection, id: String) -> Result<(), DbErr> {
        // Lock before reading, so the stored servings cannot change before the
        // counters move.
        let txn = begin_write(db).await?;
        let existing = Meal::find_by_id(id)
            .one(&txn)
            .await?
            .ok_or(DbErr::RecordNotFound("Meal not found".to_string()))?;

        if let Some(ids) = stored_recipe_ids(&existing) {
            decrement_recipe_usage(&txn, ids.iter().map(String::as_str)).await?;
        }

        existing.delete(&txn).await?;
        txn.commit().await?;
        Ok(())
    }
}

/// Returns the recipe ids in a stored meal's servings. When the stored JSON
/// cannot be read, it logs a warning naming the meal and returns `None`; the
/// caller's write still goes ahead but leaves every recipe's count unchanged.
fn stored_recipe_ids(meal: &meal::Model) -> Option<HashSet<String>> {
    match serde_json::from_str::<Vec<PersonServingDto>>(&meal.servings) {
        Ok(servings) => Some(collect_recipe_ids(&servings)),
        Err(e) => {
            tracing::warn!(
                meal_id = %meal.id,
                error = %e,
                "stored meal servings are not valid JSON; recipe plan counts left unchanged"
            );
            None
        }
    }
}

/// Collect unique recipe IDs from a list of servings
fn collect_recipe_ids(servings: &[PersonServingDto]) -> HashSet<String> {
    servings
        .iter()
        .filter_map(|s| match s {
            PersonServingDto::Recipe { recipe_id, .. } => Some(recipe_id.clone()),
            _ => None,
        })
        .collect()
}

// Each counter changes in one UPDATE statement, so the read and the
// arithmetic happen under SQLite's write lock and concurrent meal writes
// cannot lose a count. An id with no recipe row matches nothing.

/// Increments times_planned and sets last_planned for each recipe id.
async fn increment_recipe_usage<'a, C: ConnectionTrait>(
    db: &C,
    recipe_ids: impl IntoIterator<Item = &'a str>,
) -> Result<(), DbErr> {
    let now = chrono::Utc::now();
    for recipe_id in recipe_ids {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE recipes SET times_planned = times_planned + 1, last_planned = ?, \
             updated_at = ? WHERE id = ?",
            [now.into(), now.into(), recipe_id.into()],
        ))
        .await?;
    }
    Ok(())
}

/// Decrements times_planned for each recipe id, never below 0.
async fn decrement_recipe_usage<'a, C: ConnectionTrait>(
    db: &C,
    recipe_ids: impl IntoIterator<Item = &'a str>,
) -> Result<(), DbErr> {
    let now = chrono::Utc::now();
    for recipe_id in recipe_ids {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE recipes SET times_planned = MAX(times_planned - 1, 0), updated_at = ? \
             WHERE id = ?",
            [now.into(), recipe_id.into()],
        ))
        .await?;
    }
    Ok(())
}
