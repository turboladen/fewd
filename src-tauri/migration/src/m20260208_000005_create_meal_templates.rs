use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MealTemplates::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MealTemplates::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MealTemplates::Name).string().not_null())
                    .col(ColumnDef::new(MealTemplates::MealType).string().not_null())
                    .col(ColumnDef::new(MealTemplates::Servings).text().not_null())
                    .col(
                        ColumnDef::new(MealTemplates::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MealTemplates::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MealTemplates::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum MealTemplates {
    Table,
    Id,
    Name,
    MealType,
    Servings,
    CreatedAt,
    UpdatedAt,
}
