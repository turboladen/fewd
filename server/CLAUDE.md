# CLAUDE.md — Backend (server/)

Backend-specific guidance for the Rust/Axum/SeaORM server. The root `../CLAUDE.md` holds project-wide rules (cross-boundary conventions, CI, beads, session workflow) — read it too.

## Code Standards

### Rust

**Style:**

- Use `cargo fmt` (rustfmt) - runs in CI
- Use `cargo clippy` - runs in CI, fix all warnings
- Follow Rust naming conventions (snake_case for functions/variables, PascalCase for types)

**Error Handling:**

- Use `Result<T, E>` for fallible operations. Route/service errors funnel through the hand-rolled `AppError` enum (`server/src/error.rs`), whose `IntoResponse` impl maps variants (`Database`/`NotFound`/`BadRequest`/`Internal`) to HTTP status codes; add a `From<E>` impl to bubble a new error source into it. (No `anyhow`/`thiserror` in this workspace.)
- Convert SeaORM errors to appropriate HTTP error responses (`AppError` already has `From<DbErr>`)
- Log errors before returning to frontend (`AppError` logs `Database`/`Internal` via `tracing::error!` and returns a generic message so internals don't leak)
- Provide user-friendly error messages

**Patterns:**

- Service layer for business logic (`server/src/services/`)
- DTOs in `server/src/dto.rs`
- Route handlers in `server/src/routes/`
- Entities mirror database tables (`server/src/entities/`)
- Keep route handlers thin (validation + service call)

**Example Structure:**

```rust
// Route handler (thin)
pub async fn create_person(
    State(state): State<AppState>,
    Json(data): Json<CreatePersonDto>,
) -> Result<Json<person::Model>, AppError> {
    let person = PersonService::create(&state.db, data).await?;
    Ok(Json(person))
}

// Service (business logic)
impl PersonService {
    pub async fn create(
        db: &DatabaseConnection,
        data: CreatePersonDto,
    ) -> Result<person::Model, DbErr> {
        // Validation, transformation, persistence
    }
}
```

### Database/Migrations

**SeaORM Migrations:**

- One migration per entity
- Use descriptive names: `m20260118_000001_create_people.rs`
- Always implement `up` and `down`
- Test migrations can be rolled back
- JSON fields stored as TEXT

**Never edit an already-shipped migration in place.** SeaORM tracks runs by migration name in `seaql_migrations`. If you edit a migration whose name is already recorded on any environment (the dietpi deploy counts as one), that environment will skip your edit and the live schema will silently drift from what the code expects. Always add a new `m<DATE>_<NNN>_<description>.rs` for any subsequent change, even during pre-release. The dietpi box is production-equivalent for migration purposes.

**Prefer raw SQL for migration logic that introspects schema.** Helpers like `SchemaManager::has_column()` are gated on sea-orm-migration's `sqlx-sqlite` feature. The migration crate's runtime build doesn't enable that feature, so the helper panics with `"Sqlite feature is off"` in release builds while passing locally (dev-deps merge the feature in for tests). Use `PRAGMA table_info(<table>)` via `db.query_all(Statement::from_string(...))` instead — works through any plain DB connection, no feature flags. See `m20260424_000012_backfill_recipe_slugs.rs` for the pattern.

**Migrations are frozen-in-time.** Never share structs across migrations even when shapes match — m13 and m14 each define their own `Ingredient` struct despite being identical today. A future migration that mutates the shape would silently break the older one if they shared a type.

**Shared helpers between runtime ingest paths and backfill migrations live in the migration crate**, with server-side modules re-exporting. Established by `migration::ingredient_splitter` (fewd-xez) and `migration::ingredient_amount` (fewd-4i3). Server depends on migration in this workspace, so canonical helpers go down (migration), not up. Avoids drift between the runtime parser and the backfill that re-parses existing rows.

**Queries:**

- Use SeaORM query builder (type-safe)
- Filter inactive records by default
- Order results consistently
- Use transactions for multi-table updates

## Testing

### Rust Tests

**Unit Tests:**

- Test services, not commands
- Use `#[cfg(test)]` modules
- Mock database with in-memory SQLite
- Test happy path + error cases

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_create_person() {
        let db = setup_test_db().await;
        // Test logic
    }
}
```

**Integration Tests:**

- Test route handlers end-to-end
- Use `tests/` directory

### MCP integration tests via `tower::oneshot`

- rmcp's `StreamableHttpService` enforces a Host allowlist; tests must set `Host: localhost` (in defaults) or get 400 "missing Host header".
- rmcp's `initialize` is handled by `ServerHandler::get_info(&self)` — a plain `&self` method that never reads `RequestContext::extensions`. Tests asserting auth-context flow through to tools must drive the full sequence: `initialize` → capture `mcp-session-id` response header → `notifications/initialized` (202 expected) → `tools/call`. See `server/tests/mcp_auth_plumbing_test.rs` for the pattern.
- `RequestContext<RoleServer>` can't be constructed in unit tests (requires framework-internal `Peer<R>`) — exercise tools via full HTTP transport, or through wrapper helpers like `mcp::handler::authenticated_person`.

### Regression test for `#[serde(skip_serializing)]` fields

Fields like `Person.mcp_token_hash` that must never leave the server: pin with `serde_json::to_string(&model)` and assert the result does NOT contain the field name or value — covers every `Json<T>` route in `routes/*.rs` without an HTTP harness. See `mcp_token_service::tests::person_serialization_omits_mcp_token_hash` for the pattern.

### When tests are not enough

Default for ordinary runtime/route/service/handler changes is **just `cargo test`** — skip release builds. The rule below applies only to the three listed scenarios.

`cargo test` runs in dev mode and unifies dev-dependency features into the build, which can hide runtime feature-flag mismatches. For backend changes that touch:

- Database schema or migrations
- sea-orm-migration helpers (`has_column`, `has_index`, etc.)
- Cargo features in any workspace member

…also run `cargo build --release` AND smoke-test the binary against a non-empty, pre-existing DB before declaring the change done. CI runs `scripts/migration-smoke-test.sh` against pinned schema snapshots (see `.github/workflows/ci.yml` and `just smoke-test` to run it locally) — that's the production-realistic verification; reach for it before pushing rather than waiting for the dietpi deploy.

## Linting & Formatting

### Rust

```bash
# Format code (whole workspace — server + migration crates)
cargo fmt --all

# Check formatting (CI)
cargo fmt --all --check

# Lint
cargo clippy --all-targets --workspace

# Lint strict (CI)
cargo clippy --all-targets --workspace -- -D warnings
```

Use `--all` / `--workspace` flags. Bare `cargo fmt` only formats the cwd's crate, missing the migration crate; the pre-push hook (`.claude/hooks/ci-before-push.sh` runs `just ci`) checks the whole workspace, so a single-crate format will be caught — just slower than getting it right the first time.

## Key Patterns

### Backend: Service Layer Pattern

```rust
// Route handler delegates to service
routes::person::create_person()
  → services::person_service::PersonService::create()
    → entities::person::ActiveModel::insert()
```

## MCP Server

The MCP server lives at `server/src/mcp/`, mounted at `/mcp` on the existing Axum router. Transport is Streamable HTTP; Claude Desktop connects via `bunx mcp-remote` (see README).

Module layout:

- `mcp/mod.rs` — router factory + bearer-auth middleware (`Authorization: Bearer <family-member-name>`)
- `mcp/handler.rs` — `FewdMcp` struct + tool methods (one per `#[tool]`) + `ServerHandler` impl
- `mcp/lookups.rs` — shared name/id resolution helpers (`MealLookups`)
- `mcp/schemas/` — LLM-friendly input/output types, split by domain (common, recipes, meals, people, shopping, errors)

**Before extending the tool surface**, read the design principles captured in beads memory: `bd memories fewd-mcp-design-principles`. Short version: error on unknown references with actionable messages; cross-reference tools in descriptions; dual-expose important context as tool AND resource when clients vary in capability; prefer discoverable tool names; do the right thing by default server-side; iterate on output format from live session feedback; respect the domain model's expressiveness at the boundary rather than flattening it.
