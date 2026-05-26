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
pub(crate) mod diet_tags;
pub(crate) mod errors;
mod meals;
mod people;
pub(crate) mod printable;
mod prompts;
mod recipes;
mod shopping;

// Most re-exports stay scoped to `mcp` (the parent of `schemas`) — the
// schemas exist to feed handler.rs, not the rest of the crate. `printable`
// is the exception: the renderer lives in `services::printable_service` and
// imports the input types directly through this re-export.
pub(super) use common::{DateRangeParams, EmptyParams, GetRecipeParams};
pub(super) use diet_tags::{diet_tags_payload, render_diet_tags_markdown};
pub(super) use errors::CreateMealError;
pub(super) use meals::{create_meal_input_to_dto, meal_to_brief, CreateMealInput};
pub(super) use people::{
    person_to_prefs, render_family_overview, update_person_input_to_dto, UpdatePersonInput,
};
pub(super) use printable::PrintableInput;
pub(super) use prompts::WeeklyDinnerPlanArgs;
pub(super) use recipes::{
    create_recipe_input_to_dto, recipe_to_brief, recipe_to_full, CreateRecipeInput,
    ImportRecipeUrlInput, SearchRecipesParams,
};
pub(super) use shopping::shopping_item_from_dto;
