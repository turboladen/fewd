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
//! - [`errors`] — `InputError`, `ResolveError`, `CreateMealError`. Only
//!   `CreateMealError` is re-exported below (handler.rs's exhaustive
//!   match on it needs the type at runtime). `InputError` and
//!   `ResolveError` stay scoped to the submodule and the handler test
//!   module reaches them via the explicit path; that keeps clippy quiet
//!   about unused re-exports without hiding the types from tests.
//!
//! All public items used by `handler.rs`'s production code are re-exported
//! at this level so the runtime path can keep a single
//! `use super::schemas::{…}` import.

mod common;
pub(super) mod errors;
mod meals;
mod people;
mod recipes;
mod shopping;

pub use common::{DateRangeParams, EmptyParams, GetRecipeParams};
pub use errors::CreateMealError;
pub use meals::{create_meal_input_to_dto, meal_to_brief, CreateMealInput};
pub use people::{person_to_prefs, render_family_overview};
pub use recipes::{
    create_recipe_input_to_dto, recipe_to_brief, recipe_to_full, CreateRecipeInput,
    SearchRecipesParams,
};
pub use shopping::shopping_item_from_dto;
