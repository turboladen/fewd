use sea_orm::*;

use crate::dto::PersonServingDto;
use crate::dto::{CreateMealTemplateDto, UpdateMealTemplateDto};
use crate::entities::meal::Entity as Meal;
use crate::entities::meal_template::{self, Entity as MealTemplate};
use crate::services::to_json;

pub struct MealTemplateService;

impl MealTemplateService {
    pub async fn get_all(db: &DatabaseConnection) -> Result<Vec<meal_template::Model>, DbErr> {
        MealTemplate::find()
            .order_by_asc(meal_template::Column::MealType)
            .order_by_asc(meal_template::Column::Name)
            .all(db)
            .await
    }

    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: String,
    ) -> Result<Option<meal_template::Model>, DbErr> {
        MealTemplate::find_by_id(id).one(db).await
    }

    pub async fn create(
        db: &DatabaseConnection,
        data: CreateMealTemplateDto,
    ) -> Result<meal_template::Model, DbErr> {
        let now = chrono::Utc::now();

        let template = meal_template::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            name: Set(data.name),
            meal_type: Set(data.meal_type),
            servings: Set(to_json(&data.servings)?),
            created_at: Set(now),
            updated_at: Set(now),
        };

        template.insert(db).await
    }

    pub async fn update(
        db: &DatabaseConnection,
        id: String,
        data: UpdateMealTemplateDto,
    ) -> Result<meal_template::Model, DbErr> {
        let existing = MealTemplate::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Meal template not found".to_string()))?;

        let mut template: meal_template::ActiveModel = existing.into();

        if let Some(name) = data.name {
            template.name = Set(name);
        }
        if let Some(meal_type) = data.meal_type {
            template.meal_type = Set(meal_type);
        }
        if let Some(ref servings) = data.servings {
            template.servings = Set(to_json(servings)?);
        }

        template.updated_at = Set(chrono::Utc::now());

        template.update(db).await
    }

    pub async fn delete(db: &DatabaseConnection, id: String) -> Result<(), DbErr> {
        MealTemplate::delete_by_id(id).exec(db).await?;
        Ok(())
    }

    /// Create a template from an existing meal
    pub async fn create_from_meal(
        db: &DatabaseConnection,
        meal_id: String,
        name: String,
    ) -> Result<meal_template::Model, DbErr> {
        let meal = Meal::find_by_id(meal_id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Meal not found".to_string()))?;

        // Parse servings from the meal to validate the JSON, then re-serialize
        let servings: Vec<PersonServingDto> = serde_json::from_str(&meal.servings)
            .map_err(|e| DbErr::Custom(format!("Failed to parse meal servings: {}", e)))?;

        let now = chrono::Utc::now();
        let template = meal_template::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            name: Set(name),
            meal_type: Set(meal.meal_type),
            servings: Set(to_json(&servings)?),
            created_at: Set(now),
            updated_at: Set(now),
        };

        template.insert(db).await
    }
}
