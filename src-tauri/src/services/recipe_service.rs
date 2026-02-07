use sea_orm::*;

use crate::commands::recipe::{CreateRecipeDto, UpdateRecipeDto};
use crate::entities::recipe::{self, Entity as Recipe};

pub struct RecipeService;

impl RecipeService {
    pub async fn get_all(db: &DatabaseConnection) -> Result<Vec<recipe::Model>, DbErr> {
        Recipe::find()
            .order_by_asc(recipe::Column::Name)
            .all(db)
            .await
    }

    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: String,
    ) -> Result<Option<recipe::Model>, DbErr> {
        Recipe::find_by_id(id).one(db).await
    }

    pub async fn create(
        db: &DatabaseConnection,
        data: CreateRecipeDto,
    ) -> Result<recipe::Model, DbErr> {
        let now = chrono::Utc::now();

        let recipe = recipe::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            name: Set(data.name),
            description: Set(data.description),
            source: Set(data.source),
            parent_recipe_id: Set(data.parent_recipe_id),
            prep_time: Set(data.prep_time.map(|t| serde_json::to_string(&t).unwrap())),
            cook_time: Set(data.cook_time.map(|t| serde_json::to_string(&t).unwrap())),
            total_time: Set(data.total_time.map(|t| serde_json::to_string(&t).unwrap())),
            servings: Set(data.servings),
            portion_size: Set(data
                .portion_size
                .map(|p| serde_json::to_string(&p).unwrap())),
            instructions: Set(data.instructions),
            ingredients: Set(serde_json::to_string(&data.ingredients).unwrap()),
            nutrition_per_serving: Set(data
                .nutrition_per_serving
                .map(|n| serde_json::to_string(&n).unwrap())),
            tags: Set(serde_json::to_string(&data.tags).unwrap()),
            notes: Set(data.notes),
            icon: Set(data.icon),
            is_favorite: Set(false),
            times_made: Set(0),
            last_made: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };

        recipe.insert(db).await
    }

    pub async fn update(
        db: &DatabaseConnection,
        id: String,
        data: UpdateRecipeDto,
    ) -> Result<recipe::Model, DbErr> {
        let existing = Recipe::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Recipe not found".to_string()))?;

        let mut recipe: recipe::ActiveModel = existing.into();

        if let Some(name) = data.name {
            recipe.name = Set(name);
        }
        if let Some(description) = data.description {
            recipe.description = Set(Some(description));
        }
        if let Some(prep_time) = data.prep_time {
            recipe.prep_time = Set(Some(serde_json::to_string(&prep_time).unwrap()));
        }
        if let Some(cook_time) = data.cook_time {
            recipe.cook_time = Set(Some(serde_json::to_string(&cook_time).unwrap()));
        }
        if let Some(total_time) = data.total_time {
            recipe.total_time = Set(Some(serde_json::to_string(&total_time).unwrap()));
        }
        if let Some(servings) = data.servings {
            recipe.servings = Set(servings);
        }
        if let Some(portion_size) = data.portion_size {
            recipe.portion_size = Set(Some(serde_json::to_string(&portion_size).unwrap()));
        }
        if let Some(instructions) = data.instructions {
            recipe.instructions = Set(instructions);
        }
        if let Some(ingredients) = data.ingredients {
            recipe.ingredients = Set(serde_json::to_string(&ingredients).unwrap());
        }
        if let Some(nutrition) = data.nutrition_per_serving {
            recipe.nutrition_per_serving = Set(Some(serde_json::to_string(&nutrition).unwrap()));
        }
        if let Some(tags) = data.tags {
            recipe.tags = Set(serde_json::to_string(&tags).unwrap());
        }
        if let Some(notes) = data.notes {
            recipe.notes = Set(Some(notes));
        }
        if let Some(icon) = data.icon {
            recipe.icon = Set(Some(icon));
        }
        if let Some(is_favorite) = data.is_favorite {
            recipe.is_favorite = Set(is_favorite);
        }

        recipe.updated_at = Set(chrono::Utc::now());

        recipe.update(db).await
    }

    pub async fn delete(db: &DatabaseConnection, id: String) -> Result<(), DbErr> {
        Recipe::delete_by_id(id).exec(db).await?;
        Ok(())
    }

    pub async fn search(
        db: &DatabaseConnection,
        query: String,
    ) -> Result<Vec<recipe::Model>, DbErr> {
        Recipe::find()
            .filter(recipe::Column::Name.contains(&query))
            .order_by_asc(recipe::Column::Name)
            .all(db)
            .await
    }

    pub async fn toggle_favorite(
        db: &DatabaseConnection,
        id: String,
    ) -> Result<recipe::Model, DbErr> {
        let existing = Recipe::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Recipe not found".to_string()))?;

        let new_favorite = !existing.is_favorite;
        let mut recipe: recipe::ActiveModel = existing.into();
        recipe.is_favorite = Set(new_favorite);
        recipe.updated_at = Set(chrono::Utc::now());

        recipe.update(db).await
    }
}
