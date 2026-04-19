use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(DrinkRecipes::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DrinkRecipes::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(DrinkRecipes::Slug).text().not_null())
                    .col(ColumnDef::new(DrinkRecipes::Name).string().not_null())
                    .col(ColumnDef::new(DrinkRecipes::Description).string())
                    .col(ColumnDef::new(DrinkRecipes::Source).string().not_null())
                    .col(
                        ColumnDef::new(DrinkRecipes::Servings)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(ColumnDef::new(DrinkRecipes::Instructions).text().not_null())
                    .col(ColumnDef::new(DrinkRecipes::Ingredients).text().not_null())
                    .col(ColumnDef::new(DrinkRecipes::Technique).string())
                    .col(ColumnDef::new(DrinkRecipes::Glassware).string())
                    .col(ColumnDef::new(DrinkRecipes::Garnish).string())
                    .col(ColumnDef::new(DrinkRecipes::Tags).text().not_null())
                    .col(ColumnDef::new(DrinkRecipes::Notes).string())
                    .col(ColumnDef::new(DrinkRecipes::Icon).string())
                    .col(
                        ColumnDef::new(DrinkRecipes::IsFavorite)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(DrinkRecipes::IsNonAlcoholic)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(DrinkRecipes::Rating).double().null())
                    .col(
                        ColumnDef::new(DrinkRecipes::TimesMade)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(DrinkRecipes::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DrinkRecipes::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_drink_recipes_slug")
                    .table(DrinkRecipes::Table)
                    .col(DrinkRecipes::Slug)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(DrinkRecipes::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum DrinkRecipes {
    Table,
    Id,
    Slug,
    Name,
    Description,
    Source,
    Servings,
    Instructions,
    Ingredients,
    Technique,
    Glassware,
    Garnish,
    Tags,
    Notes,
    Icon,
    IsFavorite,
    IsNonAlcoholic,
    Rating,
    TimesMade,
    CreatedAt,
    UpdatedAt,
}
