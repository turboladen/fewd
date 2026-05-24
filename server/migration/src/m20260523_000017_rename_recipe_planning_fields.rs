//! Rename `recipes.times_made` → `times_planned` and `recipes.last_made` →
//! `last_planned` (fewd-5al).
//!
//! These columns are written ONLY by meal *scheduling*
//! (`MealService::create`/`update`), never by a cooking-history signal, so
//! their old names were misleading — they track planning, not cooking. The
//! rename makes the schema read honestly. A separate, deferred effort
//! (fewd-sx3) will introduce a real cooking-history concept across the UI and
//! MCP together.
//!
//! Raw `ALTER TABLE … RENAME COLUMN` (SQLite 3.25+) is used instead of the
//! sea-query alter builder to sidestep any SQLite alter-table quirks and to
//! match the raw-SQL convention established by the backfill migrations. Only
//! the `recipes` table is renamed; `drink_recipes.times_made` is intentionally
//! left alone (drinks aren't scheduled in the planner, so "planned" semantics
//! don't apply).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE recipes RENAME COLUMN times_made TO times_planned")
            .await?;
        db.execute_unprepared("ALTER TABLE recipes RENAME COLUMN last_made TO last_planned")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("ALTER TABLE recipes RENAME COLUMN times_planned TO times_made")
            .await?;
        db.execute_unprepared("ALTER TABLE recipes RENAME COLUMN last_planned TO last_made")
            .await?;
        Ok(())
    }
}
