use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        add_slug_column(manager, Recipes::Table, Recipes::Slug).await?;
        backfill_slugs(manager, "recipes").await?;
        add_slug_column(manager, DrinkRecipes::Table, DrinkRecipes::Slug).await?;
        backfill_slugs(manager, "drink_recipes").await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_slug_index(manager, "idx_drink_recipes_slug").await?;
        manager
            .alter_table(
                Table::alter()
                    .table(DrinkRecipes::Table)
                    .drop_column(DrinkRecipes::Slug)
                    .to_owned(),
            )
            .await?;

        drop_slug_index(manager, "idx_recipes_slug").await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Recipes::Table)
                    .drop_column(Recipes::Slug)
                    .to_owned(),
            )
            .await
    }
}

/// Adds a nullable `slug` column so the migration can run against a populated DB.
/// Backfill happens next, then a unique index guards against duplicates. The column
/// stays nullable at the SQL level (SQLite can't promote to NOT NULL via ALTER); the
/// application enforces non-null at write time through the SeaORM entity's `slug: String`.
async fn add_slug_column<T, C>(manager: &SchemaManager<'_>, table: T, slug: C) -> Result<(), DbErr>
where
    T: IntoIden + 'static,
    C: IntoIden + 'static,
{
    manager
        .alter_table(
            Table::alter()
                .table(table)
                .add_column(ColumnDef::new(slug).text().null())
                .to_owned(),
        )
        .await
}

/// Backfill slugs from `name` on the given table, suffixing collisions with `-2`, `-3`, ...
/// Then create a UNIQUE index so future writes can't introduce duplicates.
async fn backfill_slugs(manager: &SchemaManager<'_>, table: &str) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let backend = manager.get_database_backend();

    let rows = db
        .query_all(Statement::from_string(
            backend,
            format!("SELECT id, name FROM {} ORDER BY created_at ASC", table),
        ))
        .await?;

    let mut seen = std::collections::HashSet::<String>::new();

    for row in rows {
        let id: String = row.try_get("", "id")?;
        let name: String = row.try_get("", "name")?;
        let base = crate::slugify(&name);
        let mut candidate = base.clone();
        let mut suffix = 2u32;
        while !seen.insert(candidate.clone()) {
            candidate = format!("{}-{}", base, suffix);
            suffix += 1;
        }

        db.execute(Statement::from_sql_and_values(
            backend,
            format!("UPDATE {} SET slug = ? WHERE id = ?", table),
            [candidate.into(), id.into()],
        ))
        .await?;
    }

    // Now that every row has a slug, enforce uniqueness.
    db.execute(Statement::from_string(
        backend,
        format!(
            "CREATE UNIQUE INDEX idx_{}_slug ON {} (slug)",
            table, table
        ),
    ))
    .await?;

    Ok(())
}

async fn drop_slug_index(manager: &SchemaManager<'_>, name: &str) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute(Statement::from_string(
            manager.get_database_backend(),
            format!("DROP INDEX IF EXISTS {}", name),
        ))
        .await
        .map(|_| ())
}

#[derive(Iden)]
enum Recipes {
    Table,
    Slug,
}

#[derive(Iden)]
enum DrinkRecipes {
    Table,
    Slug,
}
