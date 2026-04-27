use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{DatabaseBackend, Statement};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ingredient_splitter::split_name_and_prep;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Walks every recipe and rewrites its ingredients JSON so any ingredient
/// whose `name` contains a comma — e.g. "garlic, minced" — has the prep
/// clause peeled off into the new `prep` field. Existing rows where prep is
/// already populated are left alone.
///
/// Recipe-author meta-prose like "Pizza sauce or crushed tomatoes seasoned
/// with salt, olive oil, and oregano" ALSO gets split on its first
/// top-level comma, producing a "garbage prep" clause. This is intentional
/// — the splitter has no reliable way to tell meta-prose from genuine
/// `name, prep` pairs, and the resulting garbage doesn't collide with real
/// prep clauses in the shopping aggregator (the aggregator keys on `name`
/// only, so a meta-prose row aggregates with itself or not at all). The
/// alternative — leaving these rows untouched — would require a heuristic
/// the live data audit didn't justify.
///
/// Scoped to `recipes` only. The dietpi prod audit on 2026-04-27 confirmed
/// `drink_recipes`, `meals.servings.adhoc_items`, and
/// `meal_templates.servings.adhoc_items` all carry zero ingredient data.
/// Once a comma'd name lands in any of those tables post-migration, the
/// in-process splitter wired into the recipe parser, the URL importer, and
/// the MCP `ingredient_in` boundary handles it on write.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let rows = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT id, ingredients FROM recipes ORDER BY id".to_string(),
            ))
            .await?;

        for row in rows {
            let id: String = row.try_get("", "id")?;
            let original: String = row.try_get("", "ingredients")?;

            let rewritten = rewrite_ingredients_json(&original).map_err(|e| {
                DbErr::Custom(format!(
                    "recipe {id} has unprocessable ingredients JSON: {e}. \
                     Fix the row manually before re-running migrations \
                     (the splitter will not silently skip corrupt data)."
                ))
            })?;

            let Some(rewritten) = rewritten else {
                continue;
            };

            db.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE recipes SET ingredients = ? WHERE id = ?".to_string(),
                [rewritten.into(), id.into()],
            ))
            .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // No-op. Re-merging name+prep is lossy — the prep clause would have
        // to be reconstructed, and there's no value in undoing the split.
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Ingredient {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prep: Option<String>,
    /// Pass-through fields. We deserialize amount/unit/notes as opaque JSON
    /// so this migration doesn't have to mirror IngredientAmountDto's shape
    /// (and stays robust if that shape evolves later).
    amount: Value,
    #[serde(default)]
    unit: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
}

/// Returns `Ok(Some(new_json))` if any ingredient was rewritten,
/// `Ok(None)` if the row is already fully split (no UPDATE needed — keeps
/// the migration cheap on already-correct DBs and idempotent across re-runs),
/// or `Err` if the row's JSON is unparseable. The caller propagates parse
/// errors as `DbErr` rather than silently skipping — corrupt data should
/// halt the migration and be fixed by hand, not be invisible.
fn rewrite_ingredients_json(raw: &str) -> Result<Option<String>, serde_json::Error> {
    let mut ingredients: Vec<Ingredient> = serde_json::from_str(raw)?;

    let mut changed = false;
    for ing in ingredients.iter_mut() {
        if ing.prep.is_some() {
            continue;
        }
        if !ing.name.contains(',') {
            continue;
        }
        let (name, prep) = split_name_and_prep(&ing.name);
        if name == ing.name && prep.is_none() {
            // Splitter declined (degenerate input). Leave row untouched.
            continue;
        }
        ing.name = name;
        ing.prep = prep;
        changed = true;
    }

    if !changed {
        return Ok(None);
    }
    Ok(Some(serde_json::to_string(&ingredients)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{ConnectionTrait, Database};

    /// Recipes table in the shape we'd find on dietpi prod (ingredients TEXT
    /// holding a JSON array). Other columns are present so the SCHEMA shape
    /// matches reality and any incidental UPDATE can't fail on missing
    /// NOT NULL fields, but we only insert id + ingredients.
    const RECIPES_SCHEMA: &str = r#"
        CREATE TABLE recipes (
            id TEXT NOT NULL PRIMARY KEY,
            name TEXT NOT NULL DEFAULT '',
            slug TEXT NOT NULL DEFAULT '',
            servings INTEGER NOT NULL DEFAULT 1,
            source TEXT NOT NULL DEFAULT 'manual',
            instructions TEXT NOT NULL DEFAULT '',
            ingredients TEXT NOT NULL DEFAULT '[]',
            tags TEXT NOT NULL DEFAULT '[]',
            is_favorite INTEGER NOT NULL DEFAULT 0,
            times_made INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT '2026-01-01',
            updated_at TEXT NOT NULL DEFAULT '2026-01-01'
        );
    "#;

    async fn legacy_db() -> sea_orm_migration::sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(RECIPES_SCHEMA).await.unwrap();
        db
    }

    async fn insert(db: &sea_orm_migration::sea_orm::DatabaseConnection, id: &str, json: &str) {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO recipes (id, ingredients) VALUES (?, ?)".to_string(),
            [id.into(), json.into()],
        ))
        .await
        .unwrap();
    }

    async fn ingredients(
        db: &sea_orm_migration::sea_orm::DatabaseConnection,
        id: &str,
    ) -> Vec<Ingredient> {
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT ingredients FROM recipes WHERE id = ?".to_string(),
                [id.into()],
            ))
            .await
            .unwrap()
            .unwrap();
        let raw: String = row.try_get("", "ingredients").unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    fn legacy_garlic() -> &'static str {
        r#"[{
            "name":"garlic, minced",
            "amount":{"type":"single","value":3.0},
            "unit":"cloves",
            "notes":null
        }]"#
    }

    #[tokio::test]
    async fn splits_existing_comma_names() {
        let db = legacy_db().await;
        insert(&db, "r1", legacy_garlic()).await;

        Migration.up(&SchemaManager::new(&db)).await.unwrap();

        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "garlic");
        assert_eq!(after[0].prep.as_deref(), Some("minced"));
    }

    #[tokio::test]
    async fn idempotent_on_rerun() {
        let db = legacy_db().await;
        insert(&db, "r1", legacy_garlic()).await;

        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let first = ingredients(&db, "r1").await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        let second = ingredients(&db, "r1").await;

        assert_eq!(first[0].name, second[0].name);
        assert_eq!(first[0].prep, second[0].prep);
    }

    #[tokio::test]
    async fn leaves_already_split_rows_alone() {
        let db = legacy_db().await;
        let pre_split = r#"[{
            "name":"garlic",
            "prep":"minced",
            "amount":{"type":"single","value":3.0},
            "unit":"cloves",
            "notes":null
        }]"#;
        insert(&db, "r1", pre_split).await;

        Migration.up(&SchemaManager::new(&db)).await.unwrap();

        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "garlic");
        assert_eq!(after[0].prep.as_deref(), Some("minced"));
    }

    #[tokio::test]
    async fn paren_aware_split_in_migration() {
        let db = legacy_db().await;
        let json = r#"[{
            "name":"pear (or Fuji apple), grated",
            "amount":{"type":"single","value":1.0},
            "unit":"Asian",
            "notes":null
        }]"#;
        insert(&db, "r1", json).await;

        Migration.up(&SchemaManager::new(&db)).await.unwrap();

        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "pear (or Fuji apple)");
        assert_eq!(after[0].prep.as_deref(), Some("grated"));
    }

    #[tokio::test]
    async fn leaves_no_comma_rows_alone() {
        let db = legacy_db().await;
        let json = r#"[{
            "name":"olive oil",
            "amount":{"type":"single","value":2.0},
            "unit":"tbsp",
            "notes":null
        }]"#;
        insert(&db, "r1", json).await;

        Migration.up(&SchemaManager::new(&db)).await.unwrap();

        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "olive oil");
        assert_eq!(after[0].prep, None);
    }

    #[tokio::test]
    async fn corrupt_ingredients_json_halts_migration() {
        // Loud failure is the contract — the operator needs a signal
        // when a row can't be processed. Silent skip would let bad data
        // sail through every subsequent boot.
        let db = legacy_db().await;
        insert(&db, "r1", "{ this is not json").await;

        let result = Migration.up(&SchemaManager::new(&db)).await;
        let err = result.expect_err("migration should fail on corrupt JSON");
        let msg = err.to_string();
        assert!(
            msg.contains("r1"),
            "error should name the bad recipe id, got {msg}"
        );
        assert!(
            msg.contains("ingredients JSON"),
            "error should explain the failure, got {msg}"
        );
    }

    #[tokio::test]
    async fn empty_ingredients_array_is_left_alone() {
        let db = legacy_db().await;
        insert(&db, "r1", "[]").await;

        Migration.up(&SchemaManager::new(&db)).await.unwrap();

        let after = ingredients(&db, "r1").await;
        assert!(after.is_empty());
    }

    #[tokio::test]
    async fn mixed_recipe_only_updates_changed_ingredients() {
        let db = legacy_db().await;
        let mixed = r#"[
            {"name":"olive oil","amount":{"type":"single","value":2.0},"unit":"tbsp","notes":null},
            {"name":"garlic, minced","amount":{"type":"single","value":3.0},"unit":"cloves","notes":null},
            {"name":"salt","amount":{"type":"single","value":1.0},"unit":"tsp","notes":null}
        ]"#;
        insert(&db, "r1", mixed).await;

        Migration.up(&SchemaManager::new(&db)).await.unwrap();

        let after = ingredients(&db, "r1").await;
        assert_eq!(after[0].name, "olive oil");
        assert_eq!(after[0].prep, None);
        assert_eq!(after[1].name, "garlic");
        assert_eq!(after[1].prep.as_deref(), Some("minced"));
        assert_eq!(after[2].name, "salt");
        assert_eq!(after[2].prep, None);
    }
}
