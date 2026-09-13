---
paths:
  - "server/migration/**"
  - "server/src/entities/**"
  - "**/Cargo.toml"
  - "scripts/migration-smoke-test.sh"
---

# Database migrations

- **Write one migration per schema change**, named `m<YYYYMMDD>_<NNNNNN>_<description>.rs` and registered in `server/migration/src/lib.rs`. Implement both `up` and `down`. JSON fields are stored as TEXT.
- **Never edit an already-shipped migration in place.** SeaORM tracks runs by migration name in `seaql_migrations`, so an environment that already ran it (the dietpi deploy counts) skips the edit and its schema silently drifts from what the code expects. Always add a new migration, even during pre-release.
- **Use raw SQL for schema introspection.** `SchemaManager::has_column()` and friends are gated on sea-orm-migration's `sqlx-sqlite` feature, which the migration crate's runtime build doesn't enable: they panic with `"Sqlite feature is off"` in release builds while passing locally, where dev-dependencies merge the feature in. Use `PRAGMA table_info(<table>)` via `db.query_all(Statement::from_string(...))`, as `m20260424_000012_backfill_recipe_slugs.rs` does.
- **Migrations are frozen in time.** Never share structs across migrations even when the shapes match: m13 and m14 each define their own `Ingredient`. A later migration that changes a shared type would silently break the earlier one.
- **Helpers shared by runtime ingest paths and backfill migrations live in the migration crate**, with server modules re-exporting them, as `migration::ingredient_splitter` and `migration::ingredient_amount` do. The server depends on the migration crate, so canonical helpers go down into it, never up.

## When tests are not enough

`cargo test` runs in dev mode and unifies dev-dependency features into the build, which can hide runtime feature-flag mismatches. For changes to schema or migrations, sea-orm-migration helpers, or Cargo features in any workspace member, also run `cargo build --release` and smoke-test the binary against a non-empty, pre-existing database: `just smoke-test` runs `scripts/migration-smoke-test.sh` against the pinned schema snapshots, the same gate CI runs.
