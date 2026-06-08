use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{DatabaseBackend, Statement};
use serde::Deserialize;

use crate::total_minutes::total_time_to_minutes;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Frozen-in-time shape for parsing the stored `total_time` JSON
/// (`{"value": i32, "unit": String}`). Local to this migration per the
/// never-share-structs-across-migrations rule — a future change to the runtime
/// `TimeValueDto` must not retroactively alter how this backfill parsed.
#[derive(Deserialize)]
struct TimeValue {
    value: i32,
    unit: String,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// Add a nullable `total_minutes` column to `recipes` and backfill it from
    /// the existing `total_time` JSON, normalizing the free-form unit to whole
    /// minutes. `search_recipes`' time filter switches to this column so a
    /// recipe authored in hours stops silently failing the filter (the old
    /// clause did `json_extract($.value)` and assumed minutes).
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Raw PRAGMA, not `manager.has_column()` — that helper is gated on the
        // sqlx-sqlite feature the migration crate doesn't enable at runtime and
        // panics in release builds.
        if !column_exists(db, "recipes", "total_minutes").await? {
            db.execute_unprepared("ALTER TABLE recipes ADD COLUMN total_minutes INTEGER")
                .await?;
        }

        // Backfill from total_time. Rows with no total_time, malformed JSON, or
        // an unrecognized unit are left NULL (not time-filterable) rather than
        // normalized to a wrong value. Idempotent: rerunning recomputes the same value.
        let rows = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT id, total_time FROM recipes WHERE total_time IS NOT NULL ORDER BY id"
                    .to_owned(),
            ))
            .await?;

        for row in rows {
            let id: String = row.try_get("", "id")?;
            let total_time: String = row.try_get("", "total_time")?;
            let Ok(tv) = serde_json::from_str::<TimeValue>(&total_time) else {
                continue;
            };
            let Some(minutes) = total_time_to_minutes(tv.value, &tv.unit) else {
                continue;
            };
            db.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE recipes SET total_minutes = ? WHERE id = ?",
                [minutes.into(), id.into()],
            ))
            .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // No-op: the column is additive and nullable, and SQLite `DROP COLUMN`
        // is version-sensitive. Leaving it is harmless on rollback.
        Ok(())
    }
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
    use sea_orm::{ConnectionTrait, Database};

    async fn seed_recipe(db: &impl ConnectionTrait, id: &str, total_time: Option<&str>) {
        // Minimal recipes table for the backfill: only the columns this
        // migration touches. Frozen local schema, not the live entity.
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO recipes (id, total_time) VALUES (?, ?)",
            [id.into(), total_time.into()],
        ))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn backfills_total_minutes_normalizing_units() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE recipes (id TEXT PRIMARY KEY, total_time TEXT, total_minutes INTEGER);",
        )
        .await
        .unwrap();

        seed_recipe(&db, "min", Some(r#"{"value":45,"unit":"minutes"}"#)).await;
        seed_recipe(&db, "hour", Some(r#"{"value":2,"unit":"hours"}"#)).await;
        seed_recipe(&db, "hr-abbr", Some(r#"{"value":1,"unit":"hr"}"#)).await;
        seed_recipe(&db, "no-time", None).await;
        seed_recipe(&db, "weird-unit", Some(r#"{"value":3,"unit":"sols"}"#)).await;
        seed_recipe(&db, "malformed", Some("not json")).await;

        // The migration's backfill loop (column already exists in this fixture).
        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();

        let lookup = |id: &'static str| {
            let db = db.clone();
            async move {
                db.query_one(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "SELECT total_minutes FROM recipes WHERE id = ?",
                    [id.into()],
                ))
                .await
                .unwrap()
                .unwrap()
                .try_get::<Option<i32>>("", "total_minutes")
                .unwrap()
            }
        };

        assert_eq!(lookup("min").await, Some(45));
        assert_eq!(lookup("hour").await, Some(120));
        assert_eq!(lookup("hr-abbr").await, Some(60));
        assert_eq!(lookup("no-time").await, None);
        assert_eq!(
            lookup("weird-unit").await,
            None,
            "unrecognized unit stays NULL"
        );
        assert_eq!(
            lookup("malformed").await,
            None,
            "unparsable total_time stays NULL"
        );

        // Idempotent: rerun changes nothing.
        Migration.up(&manager).await.unwrap();
        assert_eq!(lookup("hour").await, Some(120));
    }
}
