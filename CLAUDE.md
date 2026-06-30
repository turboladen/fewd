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

**Component-specific guidance:** backend rules live in `server/CLAUDE.md`, frontend rules in `src/CLAUDE.md` (auto-loaded by your editor's working directory). This root file holds project-wide and cross-boundary rules that apply everywhere.

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

Backend Rust standards live in `server/CLAUDE.md`; frontend TS/React + design-system standards in `src/CLAUDE.md`.

## Testing

Rust-specific testing notes (unit/integration tests, MCP integration tests, serde-skip regression tests, "when tests are not enough") live in `server/CLAUDE.md`; React testing notes live in `src/CLAUDE.md`.

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

Rust lint/format commands moved to `server/CLAUDE.md`; TypeScript/React lint/format commands moved to `src/CLAUDE.md`.

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

**`.github/workflows/ci.yml`** - Verification jobs on `pull_request` (the
push-triggered formatter lives in `auto-format.yml`, kept separate so the two
event types don't produce duplicate/skipped jobs):

- ✅ Rust: `cargo fmt --check`, `cargo clippy`, `cargo test`, plus the migration drift smoke test
- ✅ TypeScript: `dprint check`, `bun run lint`, `bun run test`
- ✅ Typos: `typos --config .typos.toml`

**`.github/workflows/auto-format.yml`** - Auto-formats code on push (every branch except `main`).

**Runner & toolchain notes (fewd-5pz, 2026-06-30):** all jobs run on
`ubuntu-latest` (migrated off `macos-latest` for cost/speed — CI has no
host-arch dependency since the dietpi deploy cross-compiles aarch64). Several
things follow from that runner choice and are easy to break:

- **Rust caching is `Swatinem/rust-cache@v2`**, not hand-rolled `actions/cache`.
  Its `workspaces: ". -> target"` is pinned deliberately: the cargo workspace
  manifest, `Cargo.lock`, and `target/` all live at the **repo root** (members
  `server`, `server/migration`), even though later steps `cd server`. Do NOT
  "correct" it to `server -> server/target` — that caches an empty dir (the
  silent no-op the old `path: server/target` cache hit).
- **`bun-version` is pinned** (`oven-sh/setup-bun@v2`, currently `1.3.14`), not
  `latest` — bumping the bun toolchain means editing that pin in `ci.yml`.
- **`bun install --frozen-lockfile`** means a stale `bun.lock` fails CI — after
  a dep change, run `bun install` and commit the refreshed `bun.lock`.
- **Typos is installed via `taiki-e/install-action@v2` (`tool: typos`)**, not
  `brew` (which isn't on ubuntu). The id is `typos`, not the crate name `typos-cli`.
- Workflow-level `permissions: contents: read` + per-job `timeout-minutes` are
  set; the `auto-format` job keeps job-level `contents: write` (job-level perms
  replace, not merge with, workflow-level) so its push still works.
- **Job names/IDs are branch-protection required status checks** — renaming a
  job silently breaks the merge gate. Don't rename casually.

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

The Backend Service Layer Pattern lives in `server/CLAUDE.md`; the Frontend Query + Mutation Pattern lives in `src/CLAUDE.md`.

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

`.beads/issues.jsonl` carries **issue rows only**. As of bd 1.0.4+, `bd export` excludes `bd remember` memory rows by default (rationale: they may hold sensitive agent context; opt back in with `--include-memories`/`--all`), and bd's auto-export (`export.auto: true` in `.beads/config.yaml`) re-writes the JSONL using the bare path — so memories never land in the committed snapshot. Auto-export is **debounced to `export.interval` (60s)** via a high-water mark in `.beads/export-state.json`: a mutation flushes on the next `bd` command run after the window elapses, not on the command itself — so the mirror can trail the DB by up to ~60s. When you need it current immediately (e.g. before committing a snapshot), force a deterministic flush with a bare `bd export -o .beads/issues.jsonl`. A second refresh trigger is the **bd pre-commit hook** (`.beads/hooks/pre-commit` → `bd hooks run pre-commit`), which re-exports the JSONL on every commit. It's wired via `core.hooksPath = .beads/hooks` in machine-local `.git/config` (not committed, so it doesn't travel with a clone) — fewd had drifted to an empty `.git/hooks` and silently no-op'd the hook; re-pointed 2026-06-10 to match the sibling beads repos. The hook exports but does **not** `git add` the JSONL, so it never auto-bundles into your commit — it just leaves a freshened (unstaged) mirror in the working tree, which you discard or let the next snapshot pick up. The Dolt DB is the source of truth for memories: `bd memories` reads from it, `bd dolt push`/`pull` syncs it, and a fresh `bd init` bootstraps it from `refs/dolt/data` on the remote. The JSONL is just an export mirror, never the memory recovery path. A PR diff should therefore never show `{"_type":"memory",...}` rows; if one does, it's a stale snapshot from before this convention — regenerate with a bare `bd export -o .beads/issues.jsonl`, don't hand-edit. (History: bd <1.0.4 wrote memory rows into the JSONL and reordered them non-deterministically on every commit — Copilot repeatedly misread that churn as "cleanup" across the fewd-2y6 series, PRs #35/#37/#39. That class of noise is gone now that memories are DB-only; decided in fewd-040.)

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
