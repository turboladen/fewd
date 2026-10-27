use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Gives every recipe stored with empty or whitespace-only instructions a
/// placeholder that asks for the steps, the same text recipe imports store
/// when the model returns none. Recipes with real instructions are not
/// rewritten, so the migration is idempotent.
//
// Recipe writes reject blank instructions, and a stored blank would block
// every later edit that resends it, such as the web form's save. The
// placeholder is copied rather than imported because migrations are frozen
// in time. Whitespace is tested in Rust so it matches the service's
// `str::trim`, which SQLite's TRIM does not.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        const PLACEHOLDER: &str = "No instructions were found. Add the steps here.";
        let db = manager.get_connection();
        let rows = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT id, instructions FROM recipes ORDER BY id".to_string(),
            ))
            .await?;
        let mut filled = 0;
        for row in rows {
            let id: String = row.try_get("", "id")?;
            let instructions: String = row.try_get("", "instructions")?;
            if !instructions.trim().is_empty() {
                continue;
            }
            db.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE recipes SET instructions = ? WHERE id = ?",
                [PLACEHOLDER.into(), id.into()],
            ))
            .await?;
            filled += 1;
        }
        // This rewrites stored recipes, so it is reported. The migration
        // crate has no tracing dependency, hence eprintln.
        if filled > 0 {
            eprintln!(
                "migration m20261026_000021: filled placeholder instructions on {filled} recipe(s) stored with blank instructions"
            );
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // No-op. A blank instruction text carried no information worth
        // restoring.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{ConnectionTrait, Database, DatabaseConnection};

    // Only the columns the migration reads and writes, with the shipped names.
    const SCHEMA: &str = "CREATE TABLE recipes (id TEXT PRIMARY KEY, instructions TEXT NOT NULL);";

    async fn db_with(rows: &[(&str, &str)]) -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(SCHEMA).await.unwrap();
        for (id, instructions) in rows {
            db.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "INSERT INTO recipes (id, instructions) VALUES (?, ?)",
                [(*id).into(), (*instructions).into()],
            ))
            .await
            .unwrap();
        }
        db
    }

    async fn instructions(db: &DatabaseConnection, id: &str) -> String {
        db.query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT instructions FROM recipes WHERE id = ?",
            [id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "instructions")
        .unwrap()
    }

    #[tokio::test]
    async fn fills_blank_instructions_and_keeps_real_ones() {
        let db = db_with(&[
            ("empty", ""),
            ("spaces", "   "),
            ("mixed", " \t\r\n"),
            ("nbsp", "\u{a0}"),
            ("real", "  Boil the pasta.\n"),
        ])
        .await;

        Migration.up(&SchemaManager::new(&db)).await.unwrap();

        let placeholder = "No instructions were found. Add the steps here.";
        for id in ["empty", "spaces", "mixed", "nbsp"] {
            assert_eq!(instructions(&db, id).await, placeholder, "{id}");
        }
        assert_eq!(instructions(&db, "real").await, "  Boil the pasta.\n");
    }

    #[tokio::test]
    async fn running_twice_changes_nothing_more() {
        let db = db_with(&[("empty", ""), ("real", "Stir.")]).await;
        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();
        Migration.up(&manager).await.unwrap();

        assert_eq!(
            instructions(&db, "empty").await,
            "No instructions were found. Add the steps here."
        );
        assert_eq!(instructions(&db, "real").await, "Stir.");
    }
}
