//! LLM-friendly JSON-schema types for MCP tool inputs and outputs, plus the
//! conversion helpers that translate between domain DTOs and these shapes.
//!
//! Organized by MCP surface area:
//!
//! - [`common`] — shared input types (`EmptyParams`, `DateRangeParams`, …),
//!   bidirectional value types (`IngredientOut`, `TimeOut`, …), and the
//!   low-level conversion helpers both directions use.
//! - [`recipes`] — recipe list/full payloads and `create_recipe` input.
//! - [`meals`] — meal list payload, `create_meal` input, and the
//!   slug/name → id resolvers.
//! - [`people`] — family-member payload + the `fewd://family/overview`
//!   Markdown renderer.
//! - [`shopping`] — shopping-list output.
//! - [`errors`] — `InputError`, `ResolveError`, `CreateMealError`.
//!
//! All public items are re-exported at this level so handler.rs can keep a
//! single `use super::schemas::{…}` import.

mod common;
mod errors;
mod meals;
mod people;
mod recipes;
mod shopping;

pub use common::{DateRangeParams, EmptyParams, GetRecipeParams, SearchParams};
pub use meals::{create_meal_input_to_dto, meal_to_brief, CreateMealInput};
pub use people::{person_to_prefs, render_family_overview};
pub use recipes::{create_recipe_input_to_dto, recipe_to_brief, recipe_to_full, CreateRecipeInput};
pub use shopping::shopping_item_from_dto;
