use sea_orm::*;

use crate::dto::{CreateDrinkRecipeDto, UpdateDrinkRecipeDto};
use crate::entities::drink_recipe::{self, Entity as DrinkRecipe};
use crate::services::to_json;

pub struct DrinkRecipeService;

impl DrinkRecipeService {
    pub async fn get_all(db: &DatabaseConnection) -> Result<Vec<drink_recipe::Model>, DbErr> {
        DrinkRecipe::find()
            .order_by_asc(drink_recipe::Column::Name)
            .all(db)
            .await
    }

    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: String,
    ) -> Result<Option<drink_recipe::Model>, DbErr> {
        DrinkRecipe::find_by_id(id).one(db).await
    }

    pub async fn get_by_slug(
        db: &DatabaseConnection,
        slug: String,
    ) -> Result<Option<drink_recipe::Model>, DbErr> {
        DrinkRecipe::find()
            .filter(drink_recipe::Column::Slug.eq(slug))
            .one(db)
            .await
    }

    pub async fn create(
        db: &DatabaseConnection,
        data: CreateDrinkRecipeDto,
    ) -> Result<drink_recipe::Model, DbErr> {
        let now = chrono::Utc::now();
        let slug = generate_unique_slug(db, &data.name).await?;

        let recipe = drink_recipe::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            slug: Set(slug),
            name: Set(data.name),
            description: Set(data.description),
            source: Set(data.source),
            source_url: Set(data.source_url),
            servings: Set(data.servings),
            instructions: Set(data.instructions),
            ingredients: Set(to_json(&data.ingredients)?),
            technique: Set(data.technique),
            glassware: Set(data.glassware),
            garnish: Set(data.garnish),
            tags: Set(to_json(&data.tags)?),
            notes: Set(data.notes),
            icon: Set(data.icon),
            is_favorite: Set(false),
            is_non_alcoholic: Set(data.is_non_alcoholic.unwrap_or(false)),
            rating: Set(None),
            times_made: Set(0),
            created_at: Set(now),
            updated_at: Set(now),
        };

        recipe.insert(db).await
    }

    pub async fn update(
        db: &DatabaseConnection,
        id: String,
        data: UpdateDrinkRecipeDto,
    ) -> Result<drink_recipe::Model, DbErr> {
        let existing = DrinkRecipe::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Drink recipe not found".to_string()))?;

        let mut recipe: drink_recipe::ActiveModel = existing.into();

        if let Some(name) = data.name {
            recipe.name = Set(name);
        }
        if let Some(description) = data.description {
            recipe.description = Set(Some(description));
        }
        if let Some(servings) = data.servings {
            recipe.servings = Set(servings);
        }
        if let Some(instructions) = data.instructions {
            recipe.instructions = Set(instructions);
        }
        if let Some(ingredients) = data.ingredients {
            recipe.ingredients = Set(to_json(&ingredients)?);
        }
        if let Some(technique) = data.technique {
            recipe.technique = Set(Some(technique));
        }
        if let Some(glassware) = data.glassware {
            recipe.glassware = Set(Some(glassware));
        }
        if let Some(garnish) = data.garnish {
            recipe.garnish = Set(Some(garnish));
        }
        if let Some(tags) = data.tags {
            recipe.tags = Set(to_json(&tags)?);
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
        if let Some(is_non_alcoholic) = data.is_non_alcoholic {
            recipe.is_non_alcoholic = Set(is_non_alcoholic);
        }
        if let Some(rating) = data.rating {
            let rounded = rating.round();
            if !(1.0..=5.0).contains(&rounded) {
                return Err(DbErr::Custom(
                    "Rating must be a whole number from 1 to 5".to_string(),
                ));
            }
            recipe.rating = Set(Some(rounded));
        }

        recipe.updated_at = Set(chrono::Utc::now());

        recipe.update(db).await
    }

    pub async fn delete(db: &DatabaseConnection, id: String) -> Result<(), DbErr> {
        DrinkRecipe::delete_by_id(id).exec(db).await?;
        Ok(())
    }

    pub async fn toggle_favorite(
        db: &DatabaseConnection,
        id: String,
    ) -> Result<drink_recipe::Model, DbErr> {
        let existing = DrinkRecipe::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Drink recipe not found".to_string()))?;

        let new_favorite = !existing.is_favorite;
        let mut recipe: drink_recipe::ActiveModel = existing.into();
        recipe.is_favorite = Set(new_favorite);
        recipe.updated_at = Set(chrono::Utc::now());

        recipe.update(db).await
    }
}

async fn generate_unique_slug(db: &DatabaseConnection, name: &str) -> Result<String, DbErr> {
    let base = migration::slugify(name);
    let mut candidate = base.clone();
    let mut suffix = 2u32;
    loop {
        let exists = DrinkRecipe::find()
            .filter(drink_recipe::Column::Slug.eq(candidate.clone()))
            .one(db)
            .await?
            .is_some();
        if !exists {
            return Ok(candidate);
        }
        candidate = format!("{}-{}", base, suffix);
        suffix += 1;
    }
}
