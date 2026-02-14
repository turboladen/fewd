use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(People::Table)
                    .add_column(ColumnDef::new(People::DrinkPreferences).text().null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(People::Table)
                    .add_column(ColumnDef::new(People::DrinkDislikes).text().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(People::Table)
                    .drop_column(People::DrinkPreferences)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(People::Table)
                    .drop_column(People::DrinkDislikes)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum People {
    Table,
    DrinkPreferences,
    DrinkDislikes,
}
