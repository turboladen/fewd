use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(People::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(People::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(People::Name).string().not_null())
                    .col(ColumnDef::new(People::Birthdate).date().not_null())
                    .col(ColumnDef::new(People::DietaryGoals).string())
                    .col(ColumnDef::new(People::Dislikes).text().not_null())
                    .col(ColumnDef::new(People::Favorites).text().not_null())
                    .col(ColumnDef::new(People::Notes).string())
                    .col(
                        ColumnDef::new(People::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(People::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(People::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(People::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum People {
    Table,
    Id,
    Name,
    Birthdate,
    DietaryGoals,
    Dislikes,
    Favorites,
    Notes,
    IsActive,
    CreatedAt,
    UpdatedAt,
}
