//! Shopping-list output types and the conversion from the aggregated DTO.

use schemars::JsonSchema;
use serde::Serialize;

use crate::dto::{AggregatedIngredientDto, IngredientSourceDto, SourceType};

use super::common::{amount_out, IngredientAmountOut};

#[derive(Debug, Serialize, JsonSchema)]
pub struct ShoppingListItem {
    pub ingredient_name: String,
    /// Aggregated total across all meals in the range, when ingredients share
    /// a compatible unit category. Null when the aggregation failed (mixed
    /// unit categories or ranges combined with singles).
    pub total_amount: Option<IngredientAmountOut>,
    pub total_unit: Option<String>,
    /// Per-meal breakdown — useful when the total is null or the user wants
    /// to know which recipe contributed what.
    pub sources: Vec<ShoppingListSource>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ShoppingListSource {
    pub amount: IngredientAmountOut,
    pub unit: String,
    /// `"recipe"` or `"adhoc"`.
    pub source_type: String,
    /// Name of the contributing recipe when `source_type == "recipe"`.
    pub source_name: Option<String>,
    pub meal_date: String,
    pub meal_type: String,
}

pub fn shopping_item_from_dto(dto: AggregatedIngredientDto) -> ShoppingListItem {
    ShoppingListItem {
        ingredient_name: dto.ingredient_name,
        total_amount: dto.total_amount.map(amount_out),
        total_unit: dto.total_unit,
        sources: dto.items.into_iter().map(shopping_source).collect(),
    }
}

fn shopping_source(s: IngredientSourceDto) -> ShoppingListSource {
    let source_type = match s.source_type {
        SourceType::Recipe => "recipe",
        SourceType::Adhoc => "adhoc",
    };
    ShoppingListSource {
        amount: amount_out(s.amount),
        unit: s.unit,
        source_type: source_type.to_string(),
        source_name: s.source_name,
        meal_date: s.meal_date,
        meal_type: s.meal_type,
    }
}
