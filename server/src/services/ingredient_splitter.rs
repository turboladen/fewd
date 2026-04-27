//! Re-exports the ingredient name/prep splitter from the migration crate.
//!
//! The canonical implementation lives in the migration crate (alongside the
//! backfill that walks every existing row through it) so the migration and
//! the runtime ingest paths can never drift. Server-side callers import
//! through this module to keep the call site readable.

pub use migration::split_name_and_prep;
