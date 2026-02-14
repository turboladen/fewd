use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "drink_recipes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source: String,
    #[sea_orm(column_type = "Text")]
    pub source_url: Option<String>,
    pub servings: i32,
    #[sea_orm(column_type = "Text")]
    pub instructions: String,
    #[sea_orm(column_type = "Text")]
    pub ingredients: String,
    pub technique: Option<String>,
    pub glassware: Option<String>,
    pub garnish: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub tags: String,
    pub notes: Option<String>,
    pub icon: Option<String>,
    pub is_favorite: bool,
    pub is_non_alcoholic: bool,
    pub rating: Option<f64>,
    pub times_made: i32,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
