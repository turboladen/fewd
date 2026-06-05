use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Canonical Title-Case meal_type values, paired with the UPPER(TRIM(...)) form used to
/// match every case/whitespace variant in one pass.
const CANONICAL: [(&str, &str); 4] = [
    ("Breakfast", "BREAKFAST"),
    ("Lunch", "LUNCH"),
    ("Dinner", "DINNER"),
    ("Snack", "SNACK"),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// Normalize `meal_type` to canonical Title-Case on every `meals` and
    /// `meal_templates` row. fewd-2pf makes the column a typed `MealType` enum, and a
    /// typed read of a non-canonical value (e.g. the legacy lowercase `"dinner"` from
    /// commit db20f56) would error — so this must run before the typed code reads any
    /// row. SeaORM applies migrations to completion at startup before serving, so the
    /// ordering holds. Both the web (`meal_templates` was never canonicalized at write
    /// time) and MCP write paths feed these tables.
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        normalize(manager, "meals").await?;
        normalize(manager, "meal_templates").await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // No-op: collapsing case/whitespace variants is not reversible, and the
        // canonical form is what every other layer already expects.
        Ok(())
    }
}

async fn normalize(manager: &SchemaManager<'_>, table: &str) -> Result<(), DbErr> {
    let db = manager.get_connection();

    // Collapse every case/whitespace variant onto its canonical form. Already-canonical
    // rows match their own UPPER(TRIM(...)) and are re-set to the same value (idempotent).
    for (canonical, upper) in CANONICAL {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            format!("UPDATE {table} SET meal_type = ? WHERE UPPER(TRIM(meal_type)) = ?"),
            [canonical.into(), upper.into()],
        ))
        .await?;
    }

    // Safety net: any value that is NOT one of the four canonical forms would crash a
    // typed read forever (the enum can't deserialize it). Coerce stragglers to 'Dinner'
    // so a stray row can't brick all meal reads. This is belt-and-suspenders — the MCP
    // path already gated meal writes through canonicalization — but `meal_templates`
    // had no such gate, so it's a real guard there.
    let upper_list = CANONICAL
        .iter()
        .map(|(_, u)| format!("'{u}'"))
        .collect::<Vec<_>>()
        .join(", ");
    let coerced = db
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("UPDATE {table} SET meal_type = 'Dinner' WHERE UPPER(TRIM(meal_type)) NOT IN ({upper_list})"),
        ))
        .await?;

    // Coercing unknown values rewrites real user data, so make it observable rather than
    // silent (this runs at startup; stderr lands in the systemd journal). The migration
    // crate intentionally has no tracing dep, hence eprintln.
    if coerced.rows_affected() > 0 {
        eprintln!(
            "migration m20260605_000018: coerced {} row(s) in `{table}` with an unrecognized meal_type to 'Dinner'",
            coerced.rows_affected(),
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database};

    /// Build a minimal `meals` table, seed mixed-case rows, run the normalize pass, and
    /// assert every row is canonical. Mirrors the in-memory-sqlite test style of the
    /// slug-backfill migration.
    #[tokio::test]
    async fn normalize_collapses_case_whitespace_and_unknowns() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE meals (id TEXT PRIMARY KEY, meal_type TEXT NOT NULL);")
            .await
            .unwrap();
        for (id, raw) in [
            ("1", "dinner"),
            ("2", "DINNER"),
            ("3", "  Dinner  "),
            ("4", "Breakfast"),
            ("5", "lunch"),
            ("6", "SNACK"),
            ("7", "brunch"), // unknown → catch-all coerces to Dinner
        ] {
            db.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "INSERT INTO meals (id, meal_type) VALUES (?, ?)",
                [id.into(), raw.into()],
            ))
            .await
            .unwrap();
        }

        let manager = SchemaManager::new(&db);
        normalize(&manager, "meals").await.unwrap();

        let rows = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT id, meal_type FROM meals ORDER BY id".to_owned(),
            ))
            .await
            .unwrap();
        let got: Vec<String> = rows
            .iter()
            .map(|r| r.try_get::<String>("", "meal_type").unwrap())
            .collect();
        assert_eq!(
            got,
            vec![
                "Dinner",
                "Dinner",
                "Dinner",
                "Breakfast",
                "Lunch",
                "Snack",
                "Dinner"
            ],
        );

        // Idempotent: a second pass changes nothing.
        normalize(&manager, "meals").await.unwrap();
        let rows2 = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT meal_type FROM meals ORDER BY id".to_owned(),
            ))
            .await
            .unwrap();
        let got2: Vec<String> = rows2
            .iter()
            .map(|r| r.try_get::<String>("", "meal_type").unwrap())
            .collect();
        assert_eq!(got, got2);
    }
}
