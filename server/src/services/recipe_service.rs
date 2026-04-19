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

    /// Look up a recipe by UUID or slug. UUID-shaped input hits the primary key first,
    /// but if no row matches (e.g., a slug happens to parse as a UUID) we fall through to
    /// the slug lookup so the record is still reachable.
    pub async fn get_by_id_or_slug(
        db: &DatabaseConnection,
        id_or_slug: String,
    ) -> Result<Option<recipe::Model>, DbErr> {
        if uuid::Uuid::parse_str(&id_or_slug).is_ok() {
            if let Some(r) = Recipe::find_by_id(id_or_slug.clone()).one(db).await? {
                return Ok(Some(r));
            }
        }
        Recipe::find()
            .filter(recipe::Column::Slug.eq(id_or_slug))
            .one(db)
            .await
    }

    pub async fn create(
        db: &DatabaseConnection,
        data: CreateRecipeDto,
    ) -> Result<recipe::Model, DbErr> {
        let now = chrono::Utc::now();
        let slug = generate_unique_slug(db, &data.name).await?;

        let recipe = recipe::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            slug: Set(slug),
            name: Set(data.name),
            description: Set(data.description),
            source: Set(data.source),
            source_url: Set(data.source_url),
            parent_recipe_id: Set(data.parent_recipe_id),
            prep_time: Set(data.prep_time.map(|t| to_json(&t)).transpose()?),
            cook_time: Set(data.cook_time.map(|t| to_json(&t)).transpose()?),
            total_time: Set(data.total_time.map(|t| to_json(&t)).transpose()?),
            servings: Set(data.servings),
            portion_size: Set(data.portion_size.map(|p| to_json(&p)).transpose()?),
            instructions: Set(data.instructions),
            ingredients: Set(to_json(&data.ingredients)?),
            nutrition_per_serving: Set(data
                .nutrition_per_serving
                .map(|n| to_json(&n))
                .transpose()?),
            tags: Set(to_json(&data.tags)?),
            notes: Set(data.notes),
            icon: Set(data.icon),
            is_favorite: Set(false),
            times_made: Set(0),
            last_made: Set(None),
            rating: Set(None),
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

/// Derive a slug from the recipe name, suffixing `-2`, `-3`, ... until unique.
/// Slug is pinned at creation; renames do not rewrite it.
async fn generate_unique_slug(db: &DatabaseConnection, name: &str) -> Result<String, DbErr> {
    let base = migration::slugify(name);
    let mut candidate = base.clone();
    let mut suffix = 2u32;
    loop {
        let exists = Recipe::find()
            .filter(recipe::Column::Slug.eq(candidate.clone()))
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
