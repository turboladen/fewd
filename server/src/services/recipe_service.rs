use sea_orm::*;

use crate::dto::{CreateRecipeDto, UpdateRecipeDto};
use crate::entities::recipe::{self, Entity as Recipe};
use crate::services::to_json;

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

    pub async fn get_by_slug(
        db: &DatabaseConnection,
        slug: String,
    ) -> Result<Option<recipe::Model>, DbErr> {
        Recipe::find()
            .filter(recipe::Column::Slug.eq(slug))
            .one(db)
            .await
    }

    pub async fn create(
        db: &DatabaseConnection,
        data: CreateRecipeDto,
    ) -> Result<recipe::Model, DbErr> {
        let now = chrono::Utc::now();
        let base_slug = migration::slugify(&data.name);

        // Serialize the JSON fields once; reuse across retries.
        let prep_time = data.prep_time.map(|t| to_json(&t)).transpose()?;
        let cook_time = data.cook_time.map(|t| to_json(&t)).transpose()?;
        let total_time = data.total_time.map(|t| to_json(&t)).transpose()?;
        let portion_size = data.portion_size.map(|p| to_json(&p)).transpose()?;
        let ingredients = to_json(&data.ingredients)?;
        let nutrition_per_serving = data
            .nutrition_per_serving
            .map(|n| to_json(&n))
            .transpose()?;
        let tags = to_json(&data.tags)?;

        // Let the DB's UNIQUE index arbitrate slug collisions: try the base slug,
        // then base-2, base-3, ... incrementing on any unique-constraint violation.
        for attempt in 1..=MAX_SLUG_ATTEMPTS {
            let candidate_slug = migration::slug::with_suffix(&base_slug, attempt);
            let model = recipe::ActiveModel {
                id: Set(uuid::Uuid::new_v4().to_string()),
                slug: Set(candidate_slug),
                name: Set(data.name.clone()),
                description: Set(data.description.clone()),
                source: Set(data.source.clone()),
                source_url: Set(data.source_url.clone()),
                parent_recipe_id: Set(data.parent_recipe_id.clone()),
                prep_time: Set(prep_time.clone()),
                cook_time: Set(cook_time.clone()),
                total_time: Set(total_time.clone()),
                servings: Set(data.servings),
                portion_size: Set(portion_size.clone()),
                instructions: Set(data.instructions.clone()),
                ingredients: Set(ingredients.clone()),
                nutrition_per_serving: Set(nutrition_per_serving.clone()),
                tags: Set(tags.clone()),
                notes: Set(data.notes.clone()),
                icon: Set(data.icon.clone()),
                is_favorite: Set(false),
                times_made: Set(0),
                last_made: Set(None),
                rating: Set(None),
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
            "Could not find a unique recipe slug after {MAX_SLUG_ATTEMPTS} attempts"
        )))
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
            recipe.prep_time = Set(Some(to_json(&prep_time)?));
        }
        if let Some(cook_time) = data.cook_time {
            recipe.cook_time = Set(Some(to_json(&cook_time)?));
        }
        if let Some(total_time) = data.total_time {
            recipe.total_time = Set(Some(to_json(&total_time)?));
        }
        if let Some(servings) = data.servings {
            recipe.servings = Set(servings);
        }
        if let Some(portion_size) = data.portion_size {
            recipe.portion_size = Set(Some(to_json(&portion_size)?));
        }
        if let Some(instructions) = data.instructions {
            recipe.instructions = Set(instructions);
        }
        if let Some(ingredients) = data.ingredients {
            recipe.ingredients = Set(to_json(&ingredients)?);
        }
        if let Some(nutrition) = data.nutrition_per_serving {
            recipe.nutrition_per_serving = Set(Some(to_json(&nutrition)?));
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

/// Cap on slug-suffix retries. `recipes` has only one UNIQUE constraint (slug),
/// so any unique violation from INSERT is a slug collision and we bump the suffix.
const MAX_SLUG_ATTEMPTS: u32 = 1000;

fn is_slug_conflict(err: &DbErr) -> bool {
    matches!(
        err.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
