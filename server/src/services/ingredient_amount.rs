//! Re-exports the amount parser + unit predicate from the migration crate
//! and adapts the kind enum to the server's `IngredientAmountDto`.
//!
//! Canonical implementation lives in `migration::ingredient_amount` so the
//! runtime parser and the backfill migration share one source of truth.

pub use migration::{try_parse_amount, AmountKind};

use crate::dto::IngredientAmountDto;

/// Parse an amount token directly into the server's DTO shape. Returns
/// `None` on garbage; callers fall back to the "no parseable amount" branch.
pub fn try_parse_amount_dto(s: &str) -> Option<IngredientAmountDto> {
    match try_parse_amount(s)? {
        AmountKind::Single(value) => Some(IngredientAmountDto::Single { value }),
        AmountKind::Range { min, max } => Some(IngredientAmountDto::Range { min, max }),
    }
}
