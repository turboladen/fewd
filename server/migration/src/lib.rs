pub use sea_orm_migration::prelude::*;

mod m20260118_000001_create_people;
mod m20260118_000002_create_recipes;
mod m20260118_000003_create_meals;
mod m20260208_000004_add_recipe_rating;
mod m20260208_000005_create_meal_templates;
mod m20260208_000006_create_settings;
mod m20260212_000007_add_recipe_source_url;
mod m20260213_000008_add_person_drink_prefs;
mod m20260213_000009_create_bar_items;
mod m20260213_000010_create_drink_recipes;
mod m20260214_000011_add_drink_recipe_source_url;
mod m20260424_000012_backfill_recipe_slugs;
mod m20260427_000013_split_ingredient_name_and_prep;
pub mod ingredient_splitter;
pub mod slug;

pub use ingredient_splitter::split_name_and_prep;
pub use slug::slugify;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260118_000001_create_people::Migration),
            Box::new(m20260118_000002_create_recipes::Migration),
            Box::new(m20260118_000003_create_meals::Migration),
            Box::new(m20260208_000004_add_recipe_rating::Migration),
            Box::new(m20260208_000005_create_meal_templates::Migration),
            Box::new(m20260208_000006_create_settings::Migration),
            Box::new(m20260212_000007_add_recipe_source_url::Migration),
            Box::new(m20260213_000008_add_person_drink_prefs::Migration),
            Box::new(m20260213_000009_create_bar_items::Migration),
            Box::new(m20260213_000010_create_drink_recipes::Migration),
            Box::new(m20260214_000011_add_drink_recipe_source_url::Migration),
            Box::new(m20260424_000012_backfill_recipe_slugs::Migration),
            Box::new(m20260427_000013_split_ingredient_name_and_prep::Migration),
        ]
    }
}
