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
        let base_slug = migration::slugify(&data.name);

        let ingredients = to_json(&data.ingredients)?;
        let tags = to_json(&data.tags)?;
        let is_non_alcoholic = data.is_non_alcoholic.unwrap_or(false);

        for attempt in 1..=MAX_SLUG_ATTEMPTS {
            let candidate_slug = migration::slug::with_suffix(&base_slug, attempt);
            let model = drink_recipe::ActiveModel {
                id: Set(uuid::Uuid::new_v4().to_string()),
                slug: Set(candidate_slug),
                name: Set(data.name.clone()),
                description: Set(data.description.clone()),
                source: Set(data.source.clone()),
                source_url: Set(data.source_url.clone()),
                servings: Set(data.servings),
                instructions: Set(data.instructions.clone()),
                ingredients: Set(ingredients.clone()),
                technique: Set(data.technique.clone()),
                glassware: Set(data.glassware.clone()),
                garnish: Set(data.garnish.clone()),
                tags: Set(tags.clone()),
                notes: Set(data.notes.clone()),
                icon: Set(data.icon.clone()),
                is_favorite: Set(false),
                is_non_alcoholic: Set(is_non_alcoholic),
                rating: Set(None),
                times_made: Set(0),
                created_at: Set(now),
                updated_at: Set(now),
            };

            match model.insert(db).await {
                Ok(r) => return Ok(r),
                Err(e) if is_slug_conflict(&e) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(DbErr::Custom(format!(
            "Could not find a unique drink recipe slug after {MAX_SLUG_ATTEMPTS} attempts"
        )))
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

const MAX_SLUG_ATTEMPTS: u32 = 1000;

fn is_slug_conflict(err: &DbErr) -> bool {
    matches!(
        err.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
