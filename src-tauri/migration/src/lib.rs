pub use sea_orm_migration::prelude::*;

mod m20260118_000001_create_people;
mod m20260118_000002_create_recipes;
mod m20260118_000003_create_meals;
mod m20260208_000004_add_recipe_rating;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260118_000001_create_people::Migration),
            Box::new(m20260118_000002_create_recipes::Migration),
            Box::new(m20260118_000003_create_meals::Migration),
            Box::new(m20260208_000004_add_recipe_rating::Migration),
        ]
    }
}
