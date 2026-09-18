---
paths:
  - "server/**"
  - "Cargo.toml"
  - "Cargo.lock"
---

# Rust server

Rust + Axum + SeaORM + SQLite. Route handlers in `server/src/routes/` stay thin (validation plus a service call) and register in `api_routes()` in `server/src/routes/mod.rs`; business logic lives in `server/src/services/`, DTOs in `server/src/dto.rs`, and entities in `server/src/entities/`. For a new entity end to end, use the `new-entity` skill.

## Error handling

- Route and service errors funnel through the hand-rolled `AppError` enum (`server/src/error.rs`), whose `IntoResponse` impl maps `Database`/`NotFound`/`BadRequest`/`Internal` to HTTP status codes. Add a `From<E>` impl to bubble a new error source into it. There is no `anyhow` or `thiserror` in this workspace.
- `AppError` logs `Database`/`Internal` via `tracing::error!` and returns a generic message, so internals don't leak.
- Service writes that validate input return `ServiceError { Validation, Database }` (`server/src/services/service_error.rs`): `Validation` maps to a 400 over HTTP and an actionable tool error over MCP.

## Tests

- Unit tests use `#[cfg(test)]` modules with in-memory SQLite; integration tests live in `server/tests/`.
- **Fields that must never leave the server** (e.g. `Person.mcp_token_hash` behind `#[serde(skip_serializing)]`): pin them with `serde_json::to_string(&model)` and assert the output contains neither the field name nor the value. See `mcp_token_service::tests::person_serialization_omits_mcp_token_hash`.
- Default verification for ordinary route, service, and handler changes is `cargo test -p fewd-server`. A change under `server/migration/` also needs `cargo test -p migration`. Bare `cargo test` from the repo root selects both packages, which the section below rules out. The release-build requirement for schema and feature changes is in the migrations rule.
- `cargo test` takes one name filter before `--`; pass several as `cargo test -p fewd-server --all-features --lib -- a b c`. Name filters match test names, not integration-test file names, so run one file with `--test <file_stem>` (e.g. `--test mcp_session_reattach_test`).

## Formatting and lint

Run these from the repo root. CI runs the same commands, with `cargo fmt --all -- --check` in place of the first:

```bash
cargo fmt --all
cargo clippy -p fewd-server --all-targets --all-features -- -D warnings
cargo clippy -p migration --all-targets -- -D warnings
```

Use `cargo fmt --all`: from `server/`, bare `cargo fmt` formats only that crate and misses `server/migration`. Run clippy and `cargo test` per package from the repo root, never with `--workspace`: a run that selects both packages unifies the migration crate's dev-dependency features into the server build, as the CI rule explains.

## Missing `dist/`

`rust-embed`'s `#[folder = "../dist"]` is checked at macro expansion. Without `dist/`, the real error (`folder '…/dist' does not exist`) is followed by misleading `no associated function named 'get'` errors for `Assets`. Fix with `mkdir -p dist && touch dist/.gitkeep`, or `bun run build`.
