# CLAUDE.md

This file provides context for AI coding assistants (Claude Code, Copilot, etc.) working on this project.

## Project Overview

Family meal planner & cocktail manager web app. Rust/Axum backend with SQLite, React frontend. See `REQUIREMENTS.md` for full specifications and `IMPLEMENTATION_PLAN.md` for build order.

**Architecture:**

- Backend: Rust + Axum + SeaORM + SQLite (in `server/`)
- Frontend: React 18 + TypeScript + Vite + TanStack Query + Tailwind (in `src/`)
- Standalone web app (previously Tauri desktop app)

**Navigation Structure:**

Top-level tabs: **Family** | **Meals** | **Recipes** | **Cocktails**
- **Meals** sub-tabs: Planner | Templates | Shopping
- **Cocktails** sub-tabs: Suggest | Recipes | My Bar

Sub-navigation uses a generic `SubNav<T>` component in `App.tsx`.

## Development Environment

**Primary Setup:**

- OS: macOS
- Editor: Neovim (primary), Zed (secondary)
- Terminal: Uses `opencode` frequently

**Required Tools:**

- Rust toolchain (via rustup)
- Bun.js (JavaScript runtime and package manager) — **always use `bun`/`bunx`, never `npm`/`npx`**
- dprint: `cargo install dprint` (code formatter)

## Project Principles

### Code Quality

**DRY (Don’t Repeat Yourself)**

- Extract shared logic into reusable functions/components
- Use SeaORM’s service layer pattern for database operations
- Share TypeScript types between components via `src/types/`

**Maintainability**

- Keep functions small and focused (single responsibility)
- Use descriptive names (prefer clarity over brevity)
- Avoid deep nesting (max 3 levels)
- Prefer composition over inheritance

**Intuitive & Ergonomic**

- Code should read like prose
- Function signatures should be self-documenting
- Error messages should be actionable
- UI components should have obvious purposes

### Documentation

**What to Document:**

- Public APIs and exported functions (brief JSDoc/Rustdoc)
- Complex algorithms or non-obvious logic
- Why decisions were made (when not obvious from code)

**What NOT to Document:**

- Obvious code (e.g., `getName()` that returns name)
- Implementation details that are clear from reading
- Redundant comments that just restate the code

**Style:**

- Concise and direct
- Focus on “why” not “what”
- Use examples for complex cases

Example:

```rust
// Good: Explains why
/// Scales ingredient amounts by servings_count to support
/// partial recipes (e.g., 0.5 servings for 2 people from 4-serving recipe)
fn scale_ingredients(recipe: &Recipe, servings_count: f64) -> Vec<Ingredient>

// Bad: Restates the obvious
/// Gets all people from the database
async fn get_all_people(db: &DatabaseConnection) -> Vec<Person>

// Better: Just the signature is enough
async fn get_all_people(db: &DatabaseConnection) -> Vec<Person>
```

## Code Standards

### Rust

**Style:**

- Use `cargo fmt` (rustfmt) - runs in CI
- Use `cargo clippy` - runs in CI, fix all warnings
- Follow Rust naming conventions (snake_case for functions/variables, PascalCase for types)

**Error Handling:**

- Use `Result<T, E>` for fallible operations; `thiserror` for libs/internal, `anyhow` for apps/public
- Convert SeaORM errors to appropriate HTTP error responses
- Log errors before returning to frontend
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

### TypeScript/React

**Style:**

- Use dprint for formatting (enforced in CI)
- ESLint rules enforced in CI
- Prefer function components with hooks
- Use TypeScript strictly (no `any`)

**Component Structure:**

```typescript
// 1. Imports
import { useState } from 'react'
import { usePeople } from '../hooks/usePeople'

// 2. Types/Interfaces (if not in src/types/)
interface Props {
  onSave: () => void
}

// 3. Component
export function MyComponent({ onSave }: Props) {
  // 3a. Hooks
  const { data } = usePeople()
  const [isOpen, setIsOpen] = useState(false)
  
  // 3b. Event handlers
  const handleClick = () => {
    setIsOpen(true)
  }
  
  // 3c. Render helpers (if needed)
  const renderItem = (item: Item) => <div>{item.name}</div>
  
  // 3d. Early returns
  if (!data) return <div>Loading...</div>
  
  // 3e. Main render
  return <div onClick={handleClick}>...</div>
}
```

**State Management:**

- TanStack Query for server state (don’t duplicate in local state)
- `useState` for local UI state
- Avoid prop drilling (composition over props)
- No Redux/Zustand needed for this app

**Naming:**

- Components: PascalCase (e.g., `FamilyManager`)
- Hooks: camelCase with `use` prefix (e.g., `usePeople`)
- Event handlers: `handle` prefix (e.g., `handleClick`)
- Boolean props/state: `is/has/should` prefix (e.g., `isOpen`)

### Frontend Design System

**Tailwind v4 (CSS-first).** There is no `tailwind.config.js` — the theme lives in an
`@theme { … }` block in `src/index.css`, the build uses the `@tailwindcss/vite` plugin
(no PostCSS/autoprefixer), and `src/index.css` starts with `@import 'tailwindcss'`.
Design tokens are defined with the v4 `@utility` directive (not `@layer components`).
Every border in the app carries an explicit `border-*` color (tokens and component
classes alike), so v4's "bare `border` defaults to `currentColor`" change is a no-op
here — no border-color compatibility shim is needed. One Preflight shim remains in
`index.css`: a `button { cursor: pointer }` rule (v4 defaults buttons to
`cursor: default`). The browser baseline is Safari 16.4+ / Chrome 111+ / Firefox 128+
(set in `vite.config.ts` `build.target`).

**Typography:** Self-hosted variable fonts in `public/fonts/`:
- Headings: Playfair Display (serif) — `--font-heading` in the `@theme` block (`font-heading`)
- Body: DM Sans (sans-serif) — `--font-sans` override in the `@theme` block

**Design Tokens (`src/index.css`, defined via `@utility`):**

| Token | Usage |
|-------|-------|
| `.btn` + `.btn-xs`/`.btn-sm`/`.btn-md` | Button sizes (include focus ring + transition) |
| `.btn-primary`/`.btn-secondary`/`.btn-outline`/`.btn-ghost`/`.btn-danger` | Button variants |
| `.input`/`.input-sm` | Text inputs, selects, textareas |
| `.card`/`.card-hover` | Content containers with shadow + rounded-xl |
| `.tag` | Rounded-full pills for labels |
| `.panel-primary`/`.panel-secondary`/`.panel-warning`/`.panel-error` | Colored card variants |

**Animation Utilities (`src/index.css`, defined via `@utility`; keyframes in `@layer utilities`):**
- `animate-fade-in`, `animate-slide-up`, `animate-slide-down`, `animate-scale-in`, `animate-backdrop` — opacity + transform, GPU-composited
- `animate-expand` — height-based accordion reveal using `grid-template-rows: 0fr → 1fr`

**Shared UI Components:**

| Component | Purpose |
|-----------|---------|
| `Icon.tsx` | SVG icon components (Heroicons paths): `IconGear`, `IconClose`, `IconCheck`, `IconPlus`, `IconSearch`, `IconTrash`, `IconEdit`, `IconStar`/`IconStarFilled`, `IconArrowLeft`/`Right`, `IconChevronUp`/`Down`/`Left`/`Right`, `IconWarning`, `IconRefresh` |
| `Toast.tsx` | `ToastProvider` context + `useToast()` hook. Wrap app in provider, call `toast('message')` in mutation callbacks. |
| `EmptyState.tsx` | Centered empty-state display. Props: `emoji`, `title`, `description`, optional `action` |
| `TagInput.tsx` | Reusable tag editor. Props: `label`, `value`, `onChange`, optional `placeholder` |
| `StarRating.tsx` | Star rating display/input with SVG stars |
| `IngredientInput.tsx` | Reusable ingredient list editor (name, amount, unit, notes). Shared by food and drink recipe forms |
| `DrinkRecipeForm.tsx` | Drink recipe add/edit form. Reuses `IngredientInput` + `TagInput`. Types live in `src/types/drinkRecipe.ts` (not the component file) to satisfy `react-refresh/only-export-components` |

**Color Palette** (`@theme` CSS variables in `src/index.css`, e.g. `--color-primary-600`):
- `primary` — earthy greens (forest/sage tones)
- `secondary` — warm terracotta/copper
- `accent` — gold/amber highlights
- `surface` — warm off-white `#FDFAF6`

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

### React Tests

**What to Test:**

- User interactions (clicks, form inputs)
- Data fetching states (loading, error, success)
- Conditional rendering

**Tools:**

- Vitest for test runner
- React Testing Library for component tests
- Mock API calls (fetch)

**Don’t Test:**

- Implementation details
- Third-party libraries
- Styling

**vitest doesn't run tsc.** Type errors in test code (e.g. factories missing newly-required fields, drifted prop types) pass silently in `bun run test`. Run `bunx tsc --noEmit` explicitly, or rely on the production build (`bun run build` chains tsc) to catch these. When expanding a shared type, sweep `src/test/factories.ts` for affected factories.

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

### Test Commands

```bash
# Rust tests
cargo test

# Frontend tests
bun run test

# Frontend tests (watch mode)
bun run test:watch
```

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

### TypeScript/React

```bash
# Format
dprint fmt

# Check formatting (CI)
dprint check

# Lint
bun run lint

# Lint fix
bun run lint:fix
```

### Typos

Uses `typos-cli` to catch typos in code and docs. Configuration in `.typos.toml`.

```bash
# Check typos
typos
```

## Running the App

### Development

```bash
# Install dependencies
bun install
cd server && cargo build && cd ..

# Run both server + frontend (recommended)
bun run dev:full

# Or run separately:
bun run dev:server   # Axum backend on port 3000
bun run dev          # Vite frontend on port 5173
```

**Dev Mode Features:**

- React hot reload (Vite)
- Axum server runs concurrently
- Frontend proxies API requests to backend

### Building

```bash
# Build frontend
bun run build

# Build server
bun run build:server

# Build both
bun run build:full
```

### Database Location

The database location is configurable via Settings → Database Location.

**Inspect Database:**

```bash
# Or use GUI tool like DB Browser for SQLite
sqlite3 <path-to-fewd.db>
```

## CI/CD (GitHub Actions)

### Workflows

**`.github/workflows/ci.yml`** - Runs on every push and PR:

- ✅ Rust: `cargo fmt --check`, `cargo clippy`, `cargo test`
- ✅ TypeScript: `dprint check`, `bun run lint`, `bun run test`
- ✅ Typos: `typos --config .typos.toml`

**`.github/workflows/auto-format.yml`** - Auto-formats code on push.

There is **no tag-triggered build workflow**. Releases are notes-only: an
annotated tag (`vYYYY-MM-DD`, with a `.N` suffix for same-day hotfixes —
`v2026-06-01`, `v2026-06-01.1`, …) plus a hand-written release via
`gh release create <tag> --title <tag> --notes-file <f> --latest`. No binaries
are attached; the dietpi box builds from source at deploy time. Tag on `main`
after the work is merged + synced with origin.

### Running CI Locally

```bash
# Full CI check (what runs in GitHub)
./scripts/ci-check.sh

# Or manually:
cargo fmt --check && cargo clippy -- -D warnings && cargo test
dprint check && bun run lint && bun run test
typos
```

### Deploying to the dietpi box

`just deploy <user>@<host>` is the whole deploy. It depends on `build-arm64`
(runs `bun run build`, then `cargo build --release --target aarch64-...`), so a
single binary push carries **frontend changes too** — the frontend is embedded
via `rust-embed` (`server/src/main.rs`: `#[folder = "../dist"]`). There is no
separate `dist/` sync; if `dist/` is stale the binary is stale.

The recipe copies `deploy/fewd.service` to **both** `/opt/fewd/` and
`/etc/systemd/system/`, then `daemon-reload` + restart — so unit-file edits
(`RUST_LOG`, `Restart=always`, `MCP_ALLOWED_HOSTS`) propagate. Don't hand-roll a
partial deploy; omitting the `/etc` copy caused the `fewd-82e` 403 regression.

## Common Tasks

### Add a New Entity

1. Create migration in `server/migration/src/`
1. Add to `server/migration/src/lib.rs`
1. Create entity in `server/src/entities/`
1. Create service in `server/src/services/`
1. Add DTOs in `server/src/dto.rs`
1. Create route handler in `server/src/routes/`
1. Register routes in `server/src/main.rs`
1. Create TypeScript types in `src/types/`
1. Create hooks in `src/hooks/`
1. Create UI component in `src/components/`

### Add a New Dependency

**Rust:**

```bash
cd server
cargo add <crate-name>
```

**Frontend:**

```bash
bun add <package-name>
```

### Debug Route Handlers

Add logging:

```rust
pub async fn my_handler(Json(data): Json<SomeDto>) -> Result<Json<Response>, AppError> {
    tracing::debug!("my_handler called with: {:?}", data);
    // ... rest of function
}
```

View logs:

- Dev mode: Check terminal running `bun run dev:full` (server output in blue)

### Update Database Schema

1. Create new migration
1. Run `bun run dev:full` (auto-applies migrations on startup)
1. If migration fails, check logs and fix
1. Test rollback: manually run `.down()` and `.up()` again

## Troubleshooting

### Common Issues

**SQLite locked errors**

- Stop dev server
- Delete database file
- Restart dev server (recreates DB)

**React not hot reloading**

- Restart dev server
- Clear Vite cache: `rm -rf node_modules/.vite`

**Rust compile errors after pulling**

```bash
cd server
cargo clean
cargo build
```

**TypeScript errors after pulling**

```bash
rm -rf node_modules bun.lockb
bun install
```

## Key Patterns

### Backend: Service Layer Pattern

```rust
// Route handler delegates to service
routes::person::create_person()
  → services::person_service::PersonService::create()
    → entities::person::ActiveModel::insert()
```

### Frontend: Query + Mutation Pattern

```typescript
// Read data
const { data } = usePeople() // React Query

// Write data
const mutation = useCreatePerson()
mutation.mutate(newPerson)
```

### Type Safety Across Boundary

```rust
// Rust DTO
#[derive(Serialize)]
struct PersonDto { name: String }
```

```typescript
// TypeScript mirror
interface PersonDto { name: string }
```

## Cross-Boundary Conventions

Invariants the type system does not enforce but production code assumes. Violate these quietly and you'll see "data's in the DB but the UI doesn't render it" bugs — the kind that take hours to diagnose because nothing errors.

### Meal type + slot encoding

`Meal.meal_type` is Title Case. The `MealPlanner` UI renders per-day cells with strict equality: `meal.meal_type === 'Dinner'`. Store `'dinner'` and the meal becomes invisible to the planner.

`Meal.order_index` is a slot number, not a sort key. `DEFAULT_MEALS` in `src/components/MealPlanner.tsx` pins the mapping: Breakfast=0, Lunch=1, Dinner=2, Snack=3. A Dinner at `order_index=0` sits at "the Breakfast slot" expectation and gets rejected on type mismatch.

Both invariants are enforced at the MCP boundary by `canonical_meal_type` and `default_order_index` in `server/src/mcp/schemas/meals.rs`. Any new write path (HTTP routes, future tools, direct SQL migrations) must do the same normalization or the meal will not render in the planner. See `fewd-2pf` for the follow-up work to make this compile-enforced via a `MealType` enum.

### CSRF protection on state-changing POST routes

State-changing POST routes (rotation, provisioning, anything that mutates server state without a JSON body in the normal client flow) must take a `Json<T>` body extractor — even an empty `#[derive(Deserialize)] struct Empty {}`. HTML form posts are CORS-simple and bypass preflight without a body type; requiring `Content-Type: application/json` forces preflight, which the locked-down CORS allowlist then rejects from non-allowed origins. DELETE is non-simple by method so it's already preflighted. See `routes::people::provision_mcp_token` for an example.

### Database path

`just dev` runs `cargo run --bin fewd-server` from the workspace root, so `DATABASE_PATH`'s default (`./data/fewd.db`) resolves to `project-root/data/fewd.db`. Don't introduce `cd server` in any run command — it'll create a parallel DB at `server/data/fewd.db` that silently drifts out of sync with the one the UI reads from.

## MCP Server

The MCP server lives at `server/src/mcp/`, mounted at `/mcp` on the existing Axum router. Transport is Streamable HTTP; Claude Desktop connects via `bunx mcp-remote` (see README).

Module layout:

- `mcp/mod.rs` — router factory + bearer-auth middleware (`Authorization: Bearer <family-member-name>`)
- `mcp/handler.rs` — `FewdMcp` struct + tool methods (one per `#[tool]`) + `ServerHandler` impl
- `mcp/lookups.rs` — shared name/id resolution helpers (`MealLookups`)
- `mcp/schemas/` — LLM-friendly input/output types, split by domain (common, recipes, meals, people, shopping, errors)

**Before extending the tool surface**, read the design principles captured in beads memory: `bd memories fewd-mcp-design-principles`. Short version: error on unknown references with actionable messages; cross-reference tools in descriptions; dual-expose important context as tool AND resource when clients vary in capability; prefer discoverable tool names; do the right thing by default server-side; iterate on output format from live session feedback; respect the domain model's expressiveness at the boundary rather than flattening it.

## Resources

- [Axum Docs](https://docs.rs/axum/latest/axum/)
- [SeaORM Docs](https://www.sea-ql.org/SeaORM/)
- [TanStack Query Docs](https://tanstack.com/query/latest)
- [Tailwind Docs](https://tailwindcss.com/docs)

## Questions?

Check:

1. `REQUIREMENTS.md` - What to build
1. `IMPLEMENTATION_PLAN.md` - How to build it
1. This file - How to maintain it
1. GitHub Issues - Known problems/features

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:ca08a54f -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

### Branch + commit naming

Branches: `fewd-<id>/<short-slug>` (e.g. `fewd-82e/mcp-host-allowlist`). Commits: conventional-commits prefix scoped to the bead — `fix(fewd-82e): ...`, `docs(fewd-2y6.3): ...`, `chore(beads): close fewd-82e after PR #34 merge`. Match the style of recent `git log` if uncertain.

### Bead closure: post-merge, not inside the fix PR

Closing a bead is a separate `chore(beads): close <id> after PR #<N> merge` commit on `main`, AFTER the fix PR is merged. Do NOT flip `status: closed` inside the fix PR — it makes `bd ready` / `bd list` inaccurate while the PR is in review. Precedent: `bc8e6f4`, `5411cc6`, `6320a91`, `734f724`. Documented at length in `.github/copilot-instructions.md`; surfaced here because Copilot reviews repeatedly suggest the wrong pattern and Claude has made the mistake too.

`.beads/issues.jsonl` carries **issue rows only**. As of bd 1.0.4+, `bd export` excludes `bd remember` memory rows by default (rationale: they may hold sensitive agent context; opt back in with `--include-memories`/`--all`), and bd's auto-export (`export.auto: true` in `.beads/config.yaml`) re-writes the JSONL using the bare path — so memories never land in the committed snapshot. Auto-export is **debounced to `export.interval` (60s)** via a high-water mark in `.beads/export-state.json`: a mutation flushes on the next `bd` command run after the window elapses, not on the command itself — so the mirror can trail the DB by up to ~60s. When you need it current immediately (e.g. before committing a snapshot), force a deterministic flush with a bare `bd export -o .beads/issues.jsonl`. The Dolt DB is the source of truth for memories: `bd memories` reads from it, `bd dolt push`/`pull` syncs it, and a fresh `bd init` bootstraps it from `refs/dolt/data` on the remote. The JSONL is just an export mirror, never the memory recovery path. A PR diff should therefore never show `{"_type":"memory",...}` rows; if one does, it's a stale snapshot from before this convention — regenerate with a bare `bd export -o .beads/issues.jsonl`, don't hand-edit. (History: bd <1.0.4 wrote memory rows into the JSONL and reordered them non-deterministically on every commit — Copilot repeatedly misread that churn as "cleanup" across the fewd-2y6 series, PRs #35/#37/#39. That class of noise is gone now that memories are DB-only; decided in fewd-040.)

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd dolt push
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END BEADS INTEGRATION -->
