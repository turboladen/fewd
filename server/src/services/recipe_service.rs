use std::collections::HashSet;

use sea_orm::sea_query::Expr;
use sea_orm::*;

use crate::dto::{CreateRecipeDto, UpdateRecipeDto};
use crate::entities::recipe::{self, Entity as Recipe};
use crate::services::to_json;

pub struct RecipeService;

/// Filter set for [`RecipeService::search_filtered`]. All fields are optional;
/// callers (the MCP handler) are responsible for refusing the all-empty case
/// before ever reaching the service. Composes at the DB layer so cost grows
/// with the *result* size, not the catalog.
#[derive(Debug, Default, Clone)]
pub struct SearchFilters {
    /// Case-insensitive substring on `name`. Empty / `*` should be normalized
    /// to `None` by the caller; the service applies this verbatim if `Some`.
    pub query: Option<String>,
    /// Lowercased exact-match tags. Multiple tags compose as AND (recipe must
    /// have every listed tag).
    pub tags: Vec<String>,
    /// Maximum recipe total time in minutes, compared against the normalized
    /// `total_minutes` column (so hour-authored recipes match correctly).
    /// Recipes with no `total_time` or an unrecognized unit are excluded.
    pub max_total_time_minutes: Option<i32>,
    pub min_rating: Option<f64>,
    pub is_favorite: Option<bool>,
    /// Recipes not planned in at least N days (or never planned).
    pub unplanned_since_days: Option<i32>,
    /// Lowercased substrings — recipe is excluded if ANY ingredient name
    /// contains ANY listed substring (case-insensitive). Already-flattened
    /// across all `excludes_for_persons` resolved in the handler.
    pub excluded_ingredient_substrings: Vec<String>,
    /// Lowercased substrings — recipe matches only if EVERY substring
    /// appears in SOME ingredient name (case-insensitive). Multiple
    /// substrings AND together; each substring may match a different
    /// ingredient. Empty Vec is a no-op (treated as "no filter on this
    /// axis").
    pub included_ingredient_substrings: Vec<String>,
}

impl SearchFilters {
    /// True when no filter has been supplied. Mirrors the equivalent guard
    /// in `SearchRecipesParams::validate_has_filter` at the MCP layer; used
    /// by `RecipeService::search_filtered` as defense-in-depth so a caller
    /// that bypasses the schema-level check still gets a loud error.
    pub fn is_empty(&self) -> bool {
        self.query.is_none()
            && self.tags.is_empty()
            && self.max_total_time_minutes.is_none()
            && self.min_rating.is_none()
            && self.is_favorite.is_none()
            && self.unplanned_since_days.is_none()
            && self.excluded_ingredient_substrings.is_empty()
            && self.included_ingredient_substrings.is_empty()
    }
}

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
        // Compute normalized minutes before `total_time` is moved into the JSON map.
        let total_minutes = data
            .total_time
            .as_ref()
            .and_then(|t| migration::total_minutes::total_time_to_minutes(t.value, &t.unit));
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
                total_minutes: Set(total_minutes),
                servings: Set(data.servings),
                portion_size: Set(portion_size.clone()),
                instructions: Set(data.instructions.clone()),
                ingredients: Set(ingredients.clone()),
                nutrition_per_serving: Set(nutrition_per_serving.clone()),
                tags: Set(tags.clone()),
                notes: Set(data.notes.clone()),
                icon: Set(data.icon.clone()),
                is_favorite: Set(false),
                times_planned: Set(0),
                last_planned: Set(None),
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
            recipe.total_minutes = Set(migration::total_minutes::total_time_to_minutes(
                total_time.value,
                &total_time.unit,
            ));
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

    /// Bounded shortlist for the MCP `list_curated_recipes` tool.
    ///
    /// Policy: every is_favorite first (never truncated — the user's explicit
    /// signal), then most-recently-planned, then top-rated. Deduped by id. Capped
    /// at `max(CURATED_CAP, favorite_count)` so a family with 50 favorites
    /// gets all 50, while a family with 5 favorites gets a 30-row blend.
    /// Within the favorites bucket, ordered by slug ascending — slugs are
    /// always lowercase by construction so the ordering is deterministic
    /// regardless of the original recipe-name capitalization (`Name` ASC
    /// would put "garlicky potatoes" after "Thai Green Curry" because
    /// SQLite's BINARY collation puts uppercase before lowercase).
    pub async fn get_curated(db: &DatabaseConnection) -> Result<Vec<recipe::Model>, DbErr> {
        let favorites = Recipe::find()
            .filter(recipe::Column::IsFavorite.eq(true))
            .order_by_asc(recipe::Column::Slug)
            .all(db)
            .await?;
        // Slug ASC tiebreaker on both buckets: same-second timestamps and
        // same ratings would otherwise let SQLite return rows in
        // implementation-defined order, so the curated shortlist could
        // shift between calls. Slug is the deterministic, stable choice
        // (always lowercase, never rewritten after creation).
        let recent = Recipe::find()
            .filter(recipe::Column::LastPlanned.is_not_null())
            .order_by_desc(recipe::Column::LastPlanned)
            .order_by_asc(recipe::Column::Slug)
            .limit(CURATED_CAP)
            .all(db)
            .await?;
        let top_rated = Recipe::find()
            .filter(recipe::Column::Rating.is_not_null())
            .order_by_desc(recipe::Column::Rating)
            .order_by_asc(recipe::Column::Slug)
            .limit(CURATED_CAP)
            .all(db)
            .await?;

        let target_total = (CURATED_CAP as usize).max(favorites.len());
        let mut seen: HashSet<String> = HashSet::with_capacity(target_total);
        let mut out: Vec<recipe::Model> = Vec::with_capacity(target_total);

        for r in favorites {
            if seen.insert(r.id.clone()) {
                out.push(r);
            }
        }
        for r in recent.into_iter().chain(top_rated) {
            if out.len() >= target_total {
                break;
            }
            if seen.insert(r.id.clone()) {
                out.push(r);
            }
        }
        Ok(out)
    }

    /// Filtered search for the MCP `search_recipes` tool. All clauses compose
    /// at the DB layer (including JSON-field filters via SQLite's `json_each`
    /// / `json_extract`).
    ///
    /// Results are ordered by `slug` ascending — slugs are always lowercase
    /// by construction so the ordering is deterministic regardless of the
    /// original recipe-name capitalization. Sorting by `Name` would use
    /// SQLite's BINARY collation which puts uppercase ASCII before lowercase
    /// (so "garlicky potatoes" would sort after "Thai Green Curry"); slug
    /// avoids the case-folding issue without needing COLLATE NOCASE.
    ///
    /// Rejects the all-default filter set with `DbErr::Custom` so a future
    /// caller that forgets to validate via `SearchRecipesParams::
    /// validate_has_filter` gets a loud error instead of silently returning
    /// the entire catalog (which is exactly what `list_curated_recipes`
    /// exists to replace).
    pub async fn search_filtered(
        db: &DatabaseConnection,
        filters: SearchFilters,
    ) -> Result<Vec<recipe::Model>, DbErr> {
        if filters.is_empty() {
            return Err(DbErr::Custom(
                "RecipeService::search_filtered called with no filters; \
                 callers must validate via SearchRecipesParams::validate_has_filter \
                 before invoking the service"
                    .to_string(),
            ));
        }

        let mut q = Recipe::find();

        if let Some(query) = filters.query.as_deref() {
            q = q.filter(recipe::Column::Name.contains(query));
        }

        for tag in &filters.tags {
            // EXISTS over json_each lets us match exact tags inside the JSON
            // array column without resorting to fragile substring matching on
            // the raw JSON text.
            q = q.filter(Expr::cust_with_values(
                "EXISTS (SELECT 1 FROM json_each(\"recipes\".\"tags\") AS je WHERE LOWER(je.value) = ?)",
                [tag.clone()],
            ));
        }

        if let Some(n) = filters.max_total_time_minutes {
            // Compare the normalized `total_minutes` column (populated from
            // total_time via migration::total_minutes, regardless of whether the
            // unit was minutes or hours). Recipes with NULL total_minutes — no
            // total_time, or an unrecognized unit — are excluded (NULL <= n is NULL).
            q = q.filter(recipe::Column::TotalMinutes.lte(n));
        }

        if let Some(min) = filters.min_rating {
            q = q.filter(recipe::Column::Rating.gte(min));
        }

        if let Some(b) = filters.is_favorite {
            q = q.filter(recipe::Column::IsFavorite.eq(b));
        }

        if let Some(days) = filters.unplanned_since_days {
            let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
            q = q.filter(
                Condition::any()
                    .add(recipe::Column::LastPlanned.is_null())
                    .add(recipe::Column::LastPlanned.lt(cutoff)),
            );
        }

        for substring in &filters.excluded_ingredient_substrings {
            // Match against the ingredient `name` field specifically — not the
            // raw JSON blob — so unrelated fields (`prep`, `unit`, `notes`)
            // don't trigger false exclusions. Substring match is intentional
            // (per the bead): "olive oil" is excluded when "olive" is disliked.
            //
            // `instr(haystack, needle) > 0` instead of LIKE because LIKE would
            // treat `%` and `_` in the needle as wildcards. A family member
            // with "100% pure olive oil" or "a_b mix" in their dislikes would
            // otherwise over-match. instr() does true substring matching.
            q = q.filter(Expr::cust_with_values(
                "NOT EXISTS (SELECT 1 FROM json_each(\"recipes\".\"ingredients\") AS ie WHERE instr(LOWER(json_extract(ie.value, '$.name')), ?) > 0)",
                [substring.clone()],
            ));
        }

        for substring in &filters.included_ingredient_substrings {
            // Mirror of the exclude loop above (drops the `NOT`); see that
            // comment for the `instr` vs `LIKE` rationale. One EXISTS per
            // substring (chained `.filter()` calls AND together) matches
            // the bead-specified semantics: every substring must appear
            // in SOME ingredient name, possibly a different one each.
            q = q.filter(Expr::cust_with_values(
                "EXISTS (SELECT 1 FROM json_each(\"recipes\".\"ingredients\") AS ie WHERE instr(LOWER(json_extract(ie.value, '$.name')), ?) > 0)",
                [substring.clone()],
            ));
        }

        q.order_by_asc(recipe::Column::Slug).all(db).await
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

/// Soft cap on `get_curated` output. Soft because favorites are never
/// truncated — if a family marks 50 favorites we return all 50.
const CURATED_CAP: u64 = 30;

fn is_slug_conflict(err: &DbErr) -> bool {
    matches!(
        err.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
