use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Recipes::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Recipes::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Recipes::Slug).text().not_null())
                    .col(ColumnDef::new(Recipes::Name).string().not_null())
                    .col(ColumnDef::new(Recipes::Description).string())
                    .col(ColumnDef::new(Recipes::Source).string().not_null())
                    .col(ColumnDef::new(Recipes::ParentRecipeId).string())
                    .col(ColumnDef::new(Recipes::PrepTime).text())
                    .col(ColumnDef::new(Recipes::CookTime).text())
                    .col(ColumnDef::new(Recipes::TotalTime).text())
                    .col(ColumnDef::new(Recipes::Servings).integer().not_null())
                    .col(ColumnDef::new(Recipes::PortionSize).text())
                    .col(ColumnDef::new(Recipes::Instructions).text().not_null())
                    .col(ColumnDef::new(Recipes::Ingredients).text().not_null())
                    .col(ColumnDef::new(Recipes::NutritionPerServing).text())
                    .col(ColumnDef::new(Recipes::Tags).text().not_null())
                    .col(ColumnDef::new(Recipes::Notes).string())
                    .col(ColumnDef::new(Recipes::Icon).string())
                    .col(
                        ColumnDef::new(Recipes::IsFavorite)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Recipes::TimesMade)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Recipes::LastMade).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(Recipes::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Recipes::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_recipes_slug")
                    .table(Recipes::Table)
                    .col(Recipes::Slug)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Recipes::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Recipes {
    Table,
    Id,
    Slug,
    Name,
    Description,
    Source,
    ParentRecipeId,
    PrepTime,
    CookTime,
    TotalTime,
    Servings,
    PortionSize,
    Instructions,
    Ingredients,
    NutritionPerServing,
    Tags,
    Notes,
    Icon,
    IsFavorite,
    TimesMade,
    LastMade,
    CreatedAt,
    UpdatedAt,
}
