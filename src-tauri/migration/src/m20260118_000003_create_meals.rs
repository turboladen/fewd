use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Meals::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Meals::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Meals::Date).date().not_null())
                    .col(ColumnDef::new(Meals::MealType).string().not_null())
                    .col(ColumnDef::new(Meals::OrderIndex).integer().not_null())
                    .col(ColumnDef::new(Meals::Servings).text().not_null())
                    .col(
                        ColumnDef::new(Meals::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Meals::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Meals::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Meals {
    Table,
    Id,
    Date,
    MealType,
    OrderIndex,
    Servings,
    CreatedAt,
    UpdatedAt,
}
