use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{DatabaseBackend, Statement};
use std::collections::HashSet;

use crate::slug::{slugify, with_suffix};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    // Backfill `slug` on existing DBs that ran the original `create_recipes`
    // and `create_drink_recipes` migrations before the slug column was added
    // in-place to those migration sources. SeaORM tracks already-run
    // migrations by name, so the in-place edit never reaches existing
    // schemas. This migration detects the missing column, adds it, populates
    // from the row's `name`, deduplicates any pre-existing duplicate slugs
    // (e.g. from a partial hand-fix), and creates the unique index. On a
    // fresh DB (where every row already has a unique slug from the create
    // migration + insert-time slug generation) every UPDATE is skipped and
    // the index creation is a no-op via `IF NOT EXISTS`.
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

    // Use raw PRAGMA instead of `manager.has_column()` — the latter is gated on
    // sea-orm-migration's `sqlx-sqlite` feature, which this crate doesn't enable
    // at runtime, so the helper panics with "Sqlite feature is off" in
    // production builds. PRAGMA works through any plain DB connection.
    if !column_exists(db, table, "slug").await? {
        db.execute_unprepared(&format!("ALTER TABLE {table} ADD COLUMN slug TEXT"))
            .await?;
    }

    // Walk every row in id order. Deterministic ordering matters because it
    // decides which row in a duplicate group keeps the base slug and which
    // ones get suffixes — without ORDER BY, SQLite's row-return order is
    // implementation-defined and two DBs with identical data could hand out
    // "pasta" / "pasta-2" to different rows on different migration runs.
    //
    // The same pass also resolves any pre-existing duplicate non-empty slugs
    // (e.g. someone hand-added the column without the unique index, then
    // inserts created collisions). For each row we pick the chosen slug from
    // either its current value (if non-empty) or `slugify(name)`, dedupe it
    // against `used` via `with_suffix`, and only `UPDATE` when the chosen
    // slug differs from what's there. That keeps the migration cheap on
    // already-correct DBs and idempotent across reruns.
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("SELECT id, name, IFNULL(slug, '') AS slug FROM {table} ORDER BY id"),
        ))
        .await?;

    let mut used: HashSet<String> = HashSet::with_capacity(rows.len());

    for row in rows {
        let id: String = row.try_get("", "id")?;
        let name: String = row.try_get("", "name")?;
        let current: String = row.try_get("", "slug")?;

        let base = if current.is_empty() {
            slugify(&name)
        } else {
            current.clone()
        };

        let mut attempt = 1u32;
        let chosen = loop {
            let candidate = with_suffix(&base, attempt);
            if !used.contains(&candidate) {
                used.insert(candidate.clone());
                break candidate;
            }
            attempt += 1;
        };

        if chosen != current {
            db.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                format!("UPDATE {table} SET slug = ? WHERE id = ?"),
                [chosen.into(), id.into()],
            ))
            .await?;
        }
    }

    db.execute_unprepared(&format!(
        "CREATE UNIQUE INDEX IF NOT EXISTS {index_name} ON {table} (slug)"
    ))
    .await?;

    Ok(())
}

async fn column_exists(
    db: &SchemaManagerConnection<'_>,
    table: &str,
    column: &str,
) -> Result<bool, DbErr> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("PRAGMA table_info({table})"),
        ))
        .await?;
    for row in rows {
        let name: String = row.try_get("", "name")?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
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
    async fn deduplicates_collisions_deterministically_by_id() {
        let db = legacy_db().await;
        insert_recipe(&db, "recipes", "r1", "Pasta").await;
        insert_recipe(&db, "recipes", "r2", "Pasta").await;
        insert_recipe(&db, "recipes", "r3", "Pasta").await;

        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();

        // Earliest id keeps the base slug; subsequent ids get incremented
        // suffixes. Without ORDER BY in the migration's SELECT, this mapping
        // is implementation-defined.
        let found = slugs(&db, "recipes").await;
        assert_eq!(
            found,
            vec![
                ("r1".into(), "pasta".into()),
                ("r2".into(), "pasta-2".into()),
                ("r3".into(), "pasta-3".into()),
            ]
        );
    }

    #[tokio::test]
    async fn falls_back_for_unsluggable_names() {
        let db = legacy_db().await;
        insert_recipe(&db, "recipes", "r1", "🍕").await;
        insert_recipe(&db, "recipes", "r2", "¿¿¿").await;

        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();

        let found = slugs(&db, "recipes").await;
        // Both fall back to "recipe"; r1 keeps the base, r2 gets the suffix.
        assert_eq!(
            found,
            vec![
                ("r1".into(), "recipe".into()),
                ("r2".into(), "recipe-2".into()),
            ]
        );
    }

    #[tokio::test]
    async fn dedupes_pre_existing_duplicate_slugs() {
        // Simulates a hand-patched DB where the column was added but the
        // unique index never was, then runtime inserts (or manual updates)
        // produced duplicate non-empty slugs. Without this pass, the
        // CREATE UNIQUE INDEX at the end of the migration would fail.
        let db = legacy_db().await;
        db.execute_unprepared("ALTER TABLE recipes ADD COLUMN slug TEXT")
            .await
            .unwrap();
        for (id, name) in [
            ("r1", "Pizza One"),
            ("r2", "Pizza Two"),
            ("r3", "Pizza Three"),
        ] {
            insert_recipe(&db, "recipes", id, name).await;
            db.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE recipes SET slug = 'pizza' WHERE id = ?",
                [id.into()],
            ))
            .await
            .unwrap();
        }

        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();

        let found = slugs(&db, "recipes").await;
        assert_eq!(
            found,
            vec![
                ("r1".into(), "pizza".into()),
                ("r2".into(), "pizza-2".into()),
                ("r3".into(), "pizza-3".into()),
            ]
        );
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
