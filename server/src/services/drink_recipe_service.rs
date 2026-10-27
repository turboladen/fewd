use sea_orm::*;

use crate::dto::{CreateDrinkRecipeDto, UpdateDrinkRecipeDto};
use crate::entities::drink_recipe::{self, Entity as DrinkRecipe};
use crate::services::normalize::{
    drop_unusable_import_ingredients, normalize_tags, required_name, trim_ingredient_names,
};
use crate::services::service_error::{whole_star_rating, ServiceError, ValidationError};
use crate::services::{begin_write, to_json};

pub struct DrinkRecipeService;

/// Repairs a drink the model produced so that create accepts it. It drops
/// nameless ingredients and replaces `servings` below 1 with 1, logging a
/// warning for each repair. Call it on every imported or suggested drink.
pub fn sanitize_generated_drink(dto: &mut CreateDrinkRecipeDto) {
    drop_unusable_import_ingredients(&dto.name, &mut dto.ingredients);
    if dto.servings < 1 {
        tracing::warn!(
            drink = %dto.name,
            servings = dto.servings,
            "replacing unusable servings in generated drink"
        );
        dto.servings = 1;
    }
}

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
        mut data: CreateDrinkRecipeDto,
    ) -> Result<drink_recipe::Model, ServiceError> {
        let name = required_name("name", data.name)?;
        if data.servings < 1 {
            return Err(ValidationError::NonPositiveServings(data.servings).into());
        }
        trim_ingredient_names("ingredients", &mut data.ingredients)?;
        let now = chrono::Utc::now();
        let base_slug = migration::slugify(&name);

        let ingredients = to_json(&data.ingredients)?;
        let tags = to_json(&normalize_tags(data.tags))?;
        let is_non_alcoholic = data.is_non_alcoholic.unwrap_or(false);

        for attempt in 1..=MAX_SLUG_ATTEMPTS {
            let candidate_slug = migration::slug::with_suffix(&base_slug, attempt);
            let model = drink_recipe::ActiveModel {
                id: Set(uuid::Uuid::new_v4().to_string()),
                slug: Set(candidate_slug),
                name: Set(name.clone()),
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
                Err(e) => return Err(e.into()),
            }
        }
        Err(DbErr::Custom(format!(
            "Could not find a unique drink recipe slug after {MAX_SLUG_ATTEMPTS} attempts"
        ))
        .into())
    }

    pub async fn update(
        db: &DatabaseConnection,
        id: String,
        mut data: UpdateDrinkRecipeDto,
    ) -> Result<drink_recipe::Model, ServiceError> {
        // No check below depends on the stored row, so a bad value answers
        // a validation error even when the id names no drink.
        let name = data.name.map(|n| required_name("name", n)).transpose()?;
        if let Some(servings) = data.servings.filter(|&s| s < 1) {
            return Err(ValidationError::NonPositiveServings(servings).into());
        }
        if let Some(ingredients) = &mut data.ingredients {
            trim_ingredient_names("ingredients", ingredients)?;
        }
        let rating = data
            .rating
            .map(|rating| rating.map(whole_star_rating).transpose())
            .transpose()?;
        let existing = DrinkRecipe::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Drink recipe not found".to_string()))?;

        let mut recipe: drink_recipe::ActiveModel = existing.into();

        if let Some(name) = name {
            recipe.name = Set(name);
        }
        if let Some(description) = data.description {
            recipe.description = Set(description);
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
            recipe.technique = Set(technique);
        }
        if let Some(glassware) = data.glassware {
            recipe.glassware = Set(glassware);
        }
        if let Some(garnish) = data.garnish {
            recipe.garnish = Set(garnish);
        }
        if let Some(tags) = data.tags {
            recipe.tags = Set(to_json(&normalize_tags(tags))?);
        }
        if let Some(notes) = data.notes {
            recipe.notes = Set(notes);
        }
        if let Some(icon) = data.icon {
            recipe.icon = Set(icon);
        }
        if let Some(is_favorite) = data.is_favorite {
            recipe.is_favorite = Set(is_favorite);
        }
        if let Some(is_non_alcoholic) = data.is_non_alcoholic {
            recipe.is_non_alcoholic = Set(is_non_alcoholic);
        }
        if let Some(rating) = rating {
            recipe.rating = Set(rating);
        }

        recipe.updated_at = Set(chrono::Utc::now());

        Ok(recipe.update(db).await?)
    }

    pub async fn delete(db: &DatabaseConnection, id: String) -> Result<(), DbErr> {
        DrinkRecipe::delete_by_id(id).exec(db).await?;
        Ok(())
    }

    /// Flips the drink's favorite flag and returns the updated row.
    pub async fn toggle_favorite(
        db: &DatabaseConnection,
        id: String,
    ) -> Result<drink_recipe::Model, DbErr> {
        // The flip is one UPDATE, so concurrent toggles all count, and the
        // read-back inside the transaction sees this call's result.
        let txn = begin_write(db).await?;
        let flipped = txn
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE drink_recipes SET is_favorite = NOT is_favorite, updated_at = ? \
                 WHERE id = ?",
                [chrono::Utc::now().into(), id.clone().into()],
            ))
            .await?;
        if flipped.rows_affected() == 0 {
            return Err(DbErr::RecordNotFound("Drink recipe not found".to_string()));
        }
        let drink = DrinkRecipe::find_by_id(id)
            .one(&txn)
            .await?
            .ok_or(DbErr::RecordNotFound("Drink recipe not found".to_string()))?;
        txn.commit().await?;
        Ok(drink)
    }
}

const MAX_SLUG_ATTEMPTS: u32 = 1000;

fn is_slug_conflict(err: &DbErr) -> bool {
    matches!(
        err.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
