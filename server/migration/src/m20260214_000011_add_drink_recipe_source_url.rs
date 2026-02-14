use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(DrinkRecipes::Table)
                    .add_column(ColumnDef::new(DrinkRecipes::SourceUrl).text().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(DrinkRecipes::Table)
                    .drop_column(DrinkRecipes::SourceUrl)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum DrinkRecipes {
    Table,
    SourceUrl,
}
