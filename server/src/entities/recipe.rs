use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "recipes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source: String,
    pub parent_recipe_id: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub prep_time: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub cook_time: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub total_time: Option<String>,
    pub servings: i32,
    #[sea_orm(column_type = "Text")]
    pub portion_size: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub instructions: String,
    #[sea_orm(column_type = "Text")]
    pub ingredients: String,
    #[sea_orm(column_type = "Text")]
    pub nutrition_per_serving: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub tags: String,
    pub notes: Option<String>,
    pub icon: Option<String>,
    pub is_favorite: bool,
    pub times_made: i32,
    pub last_made: Option<DateTimeUtc>,
    pub rating: Option<f64>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
