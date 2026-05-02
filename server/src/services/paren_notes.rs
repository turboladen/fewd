//! Re-exports the size-info parenthetical peeler from the migration crate.
//!
//! Canonical implementation lives in `migration::paren_notes` so the runtime
//! parser and the backfill migration share one source of truth — same pattern
//! as `ingredient_amount` and `ingredient_splitter`. See fewd-i47.

pub use migration::peel_size_paren;
