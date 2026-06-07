pub use sea_orm_migration::prelude::*;

pub mod ingredient_amount;
pub mod ingredient_splitter;
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
mod m20260428_000014_reparse_misbucketed_ingredients;
mod m20260429_000015_peel_size_parens_from_ingredient_name;
mod m20260509_000016_add_mcp_token_to_people;
mod m20260523_000017_rename_recipe_planning_fields;
mod m20260605_000018_normalize_meal_types;
pub mod paren_notes;
pub mod slug;

pub use ingredient_amount::{is_known_unit, try_parse_amount, try_parse_amount_json, AmountKind};
pub use ingredient_splitter::{first_top_level_or, split_name_and_prep};
pub use paren_notes::peel_size_paren;
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
            Box::new(m20260428_000014_reparse_misbucketed_ingredients::Migration),
            Box::new(m20260429_000015_peel_size_parens_from_ingredient_name::Migration),
            Box::new(m20260509_000016_add_mcp_token_to_people::Migration),
            Box::new(m20260523_000017_rename_recipe_planning_fields::Migration),
            Box::new(m20260605_000018_normalize_meal_types::Migration),
        ]
    }
}
