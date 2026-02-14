use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BarItems::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(BarItems::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(BarItems::Name).string().not_null())
                    .col(ColumnDef::new(BarItems::Category).string().not_null())
                    .col(
                        ColumnDef::new(BarItems::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BarItems::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum BarItems {
    Table,
    Id,
    Name,
    Category,
    CreatedAt,
}
