use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{DatabaseBackend, Statement};
use std::collections::HashSet;

use crate::slug::{slugify, with_suffix};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    // Backfill `slug` on existing DBs that ran the original `create_recipes` and
    // `create_drink_recipes` migrations before the slug column was added in-place
    // to those migration sources. SeaORM tracks already-run migrations by name,
    // so the in-place edit never reaches existing schemas. This migration
    // detects the missing column, adds it, populates from the row's `name`, and
    // creates the unique index. On a fresh DB (where the column is already
    // present from the create migration) every step is a no-op.
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        backfill_table(manager, "recipes", "idx_recipes_slug").await?;
        backfill_table(manager, "drink_recipes", "idx_drink_recipes_slug").await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // No-op. Dropping the slug column on existing prod DBs would orphan
        // every shareable recipe URL in the wild.
        Ok(())
    }
}

async fn backfill_table(
    manager: &SchemaManager<'_>,
    table: &str,
    index_name: &str,
) -> Result<(), DbErr> {
    let db = manager.get_connection();

    if !manager.has_column(table, "slug").await? {
        db.execute_unprepared(&format!("ALTER TABLE {table} ADD COLUMN slug TEXT"))
            .await?;
    }

    let existing = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("SELECT slug FROM {table} WHERE slug IS NOT NULL AND slug != ''"),
        ))
        .await?;
    let mut used: HashSet<String> = HashSet::with_capacity(existing.len());
    for row in existing {
        used.insert(row.try_get("", "slug")?);
    }

    let to_backfill = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("SELECT id, name FROM {table} WHERE slug IS NULL OR slug = ''"),
        ))
        .await?;

    for row in to_backfill {
        let id: String = row.try_get("", "id")?;
        let name: String = row.try_get("", "name")?;
        let base = slugify(&name);

        let mut attempt = 1u32;
        let chosen = loop {
            let candidate = with_suffix(&base, attempt);
            if !used.contains(&candidate) {
                used.insert(candidate.clone());
                break candidate;
            }
            attempt += 1;
        };

        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            format!("UPDATE {table} SET slug = ? WHERE id = ?"),
            [chosen.into(), id.into()],
        ))
        .await?;
    }

    db.execute_unprepared(&format!(
        "CREATE UNIQUE INDEX IF NOT EXISTS {index_name} ON {table} (slug)"
    ))
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{ConnectionTrait, Database};

    // Minimum legacy schema — what the original `create_recipes` / `create_drink_recipes`
    // migrations produced before commit 9d46bed added the slug column in-place.
    const LEGACY_RECIPES: &str = r#"
        CREATE TABLE recipes (
            id TEXT NOT NULL PRIMARY KEY,
            name TEXT NOT NULL,
            servings INTEGER NOT NULL DEFAULT 1,
            source TEXT NOT NULL DEFAULT 'manual',
            instructions TEXT NOT NULL DEFAULT '[]',
            ingredients TEXT NOT NULL DEFAULT '[]',
            tags TEXT NOT NULL DEFAULT '[]',
            is_favorite INTEGER NOT NULL DEFAULT 0,
            times_made INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT '2026-01-01',
            updated_at TEXT NOT NULL DEFAULT '2026-01-01'
        );
    "#;
    const LEGACY_DRINK_RECIPES: &str = r#"
        CREATE TABLE drink_recipes (
            id TEXT NOT NULL PRIMARY KEY,
            name TEXT NOT NULL,
            servings INTEGER NOT NULL DEFAULT 1,
            source TEXT NOT NULL DEFAULT 'manual',
            instructions TEXT NOT NULL DEFAULT '[]',
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
        db.execute_unprepared(LEGACY_RECIPES).await.unwrap();
        db.execute_unprepared(LEGACY_DRINK_RECIPES).await.unwrap();
        db
    }

    async fn insert_recipe(
        db: &sea_orm_migration::sea_orm::DatabaseConnection,
        table: &str,
        id: &str,
        name: &str,
    ) {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            format!("INSERT INTO {table} (id, name) VALUES (?, ?)"),
            [id.into(), name.into()],
        ))
        .await
        .unwrap();
    }

    async fn slugs(
        db: &sea_orm_migration::sea_orm::DatabaseConnection,
        table: &str,
    ) -> Vec<(String, String)> {
        db.query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("SELECT id, slug FROM {table} ORDER BY id"),
        ))
        .await
        .unwrap()
        .into_iter()
        .map(|r| (r.try_get("", "id").unwrap(), r.try_get("", "slug").unwrap()))
        .collect()
    }

    #[tokio::test]
    async fn adds_column_and_backfills_slugs() {
        let db = legacy_db().await;
        insert_recipe(&db, "recipes", "r1", "Pizza Margherita").await;
        insert_recipe(&db, "recipes", "r2", "Crème Brûlée").await;
        insert_recipe(&db, "drink_recipes", "d1", "Old Fashioned").await;

        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();

        let recipes = slugs(&db, "recipes").await;
        assert_eq!(
            recipes,
            vec![
                ("r1".into(), "pizza-margherita".into()),
                ("r2".into(), "creme-brulee".into()),
            ]
        );

        let drinks = slugs(&db, "drink_recipes").await;
        assert_eq!(drinks, vec![("d1".into(), "old-fashioned".into())]);
    }

    #[tokio::test]
    async fn deduplicates_collisions_with_numeric_suffix() {
        let db = legacy_db().await;
        insert_recipe(&db, "recipes", "r1", "Pasta").await;
        insert_recipe(&db, "recipes", "r2", "Pasta").await;
        insert_recipe(&db, "recipes", "r3", "Pasta").await;

        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();

        let mut found: Vec<String> = slugs(&db, "recipes")
            .await
            .into_iter()
            .map(|(_, s)| s)
            .collect();
        found.sort();
        assert_eq!(found, vec!["pasta", "pasta-2", "pasta-3"]);
    }

    #[tokio::test]
    async fn falls_back_for_unsluggable_names() {
        let db = legacy_db().await;
        insert_recipe(&db, "recipes", "r1", "🍕").await;
        insert_recipe(&db, "recipes", "r2", "¿¿¿").await;

        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();

        let mut found: Vec<String> = slugs(&db, "recipes")
            .await
            .into_iter()
            .map(|(_, s)| s)
            .collect();
        found.sort();
        // Both fall back to "recipe", deduped via with_suffix.
        assert_eq!(found, vec!["recipe", "recipe-2"]);
    }

    #[tokio::test]
    async fn preserves_existing_slugs_when_already_populated() {
        let db = legacy_db().await;
        insert_recipe(&db, "recipes", "r1", "Pizza").await;
        // Add the column ourselves and pre-populate one row to simulate a
        // partially-migrated DB (e.g. someone hand-fixed it).
        db.execute_unprepared("ALTER TABLE recipes ADD COLUMN slug TEXT")
            .await
            .unwrap();
        db.execute_unprepared("UPDATE recipes SET slug = 'custom-name' WHERE id = 'r1'")
            .await
            .unwrap();
        insert_recipe(&db, "recipes", "r2", "Pizza").await;

        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();

        let found = slugs(&db, "recipes").await;
        assert_eq!(
            found,
            vec![
                ("r1".into(), "custom-name".into()),
                // r2 must NOT collide with r1's "custom-name" — but more importantly,
                // it gets a real slug from its name.
                ("r2".into(), "pizza".into()),
            ]
        );
    }

    #[tokio::test]
    async fn is_idempotent() {
        let db = legacy_db().await;
        insert_recipe(&db, "recipes", "r1", "Pasta").await;
        insert_recipe(&db, "recipes", "r2", "Pasta").await;

        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();
        // Second run must not regenerate / re-collide / fail on the unique index.
        Migration.up(&manager).await.unwrap();

        let mut found: Vec<String> = slugs(&db, "recipes")
            .await
            .into_iter()
            .map(|(_, s)| s)
            .collect();
        found.sort();
        assert_eq!(found, vec!["pasta", "pasta-2"]);
    }

    #[tokio::test]
    async fn enforces_uniqueness_after_backfill() {
        let db = legacy_db().await;
        insert_recipe(&db, "recipes", "r1", "Soup").await;

        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();

        // The unique index must reject a duplicate slug.
        let dup = db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "INSERT INTO recipes (id, name, slug) VALUES (?, ?, ?)",
                ["r2".into(), "Soup".into(), "soup".into()],
            ))
            .await;
        assert!(dup.is_err(), "unique index should reject duplicate slug");
    }
}
