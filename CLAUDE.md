# CLAUDE.md

This file provides context for AI coding assistants (Claude Code, Copilot, etc.) working on this project.

## Project Overview

Family meal planner desktop app built with Tauri (Rust backend + React frontend). Stores data locally in SQLite. See `REQUIREMENTS.md` for full specifications and `IMPLEMENTATION_PLAN.md` for build order.

**Architecture:**

- Backend: Rust + Tauri 2 + SeaORM + SQLite
- Frontend: React 18 + TypeScript + Vite + TanStack Query + Tailwind
- Desktop app for macOS (primary), Windows, Linux

## Development Environment

**Primary Setup:**

- OS: macOS
- Editor: Neovim (primary), Zed (secondary)
- Terminal: Uses `opencode` frequently

**Required Tools:**

- Rust toolchain (via rustup)
- Bun.js (JavaScript runtime and package manager)
- Tauri CLI: `cargo install tauri-cli`
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
- Convert SeaORM errors to Strings at command boundary
- Log errors before returning to frontend
- Provide user-friendly error messages

**Patterns:**

- Service layer for business logic (`src/services/`)
- DTOs for Tauri commands (`src/commands/`)
- Entities mirror database tables (`src/entities/`)
- Keep commands thin (validation + service call)

**Example Structure:**

```rust
// Command (thin)
#[tauri::command]
pub async fn create_person(
    state: State<'_, AppState>,
    data: CreatePersonDto,
) -> Result<person::Model, String> {
    PersonService::create(&state.db, data)
        .await
        .map_err(|e| {
            eprintln!("Failed to create person: {}", e);
            format!("Could not create person: {}", e)
        })
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

### Database/Migrations

**SeaORM Migrations:**

- One migration per entity
- Use descriptive names: `m20260118_000001_create_people.rs`
- Always implement `up` and `down`
- Test migrations can be rolled back
- JSON fields stored as TEXT

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

- Test Tauri commands end-to-end
- Use `tests/` directory

### React Tests

**What to Test:**

- User interactions (clicks, form inputs)
- Data fetching states (loading, error, success)
- Conditional rendering

**Tools:**

- Vitest for test runner
- React Testing Library for component tests
- Mock Tauri API calls

**Don’t Test:**

- Implementation details
- Third-party libraries
- Styling

### Test Commands

```bash
# Rust tests
cargo test

# Frontend tests
npm test

# All tests (used in CI)
npm run test:all
```

## Linting & Formatting

### Rust

```bash
# Format code
cargo fmt

# Check formatting (CI)
cargo fmt --check

# Lint
cargo clippy

# Lint strict (CI)
cargo clippy -- -D warnings
```

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

Uses `typos-cli` to catch typos in code and docs.

```bash
# Install (macOS)
brew install typos-cli

# Check typos
typos

# Check with CI config
typos --config .typos.toml
```

**Configuration:** `.typos.toml`

```toml
[default]
extend-ignore-re = [
  "uuid",  # UUIDs often flagged as typos
]

[files]
extend-exclude = [
  "target/",
  "node_modules/",
  "dist/",
  "*.db",
]
```

## Running the App

### Development

```bash
# Install dependencies
bun install
cd src-tauri && cargo build && cd ..

# Run dev mode (hot reload for both Rust and React)
bun run tauri dev
```

**Dev Mode Features:**

- React hot reload (Vite)
- Rust recompiles on file change
- Opens desktop window automatically
- DevTools available (right-click → Inspect)

### Building

```bash
# Build for current architecture only
bun run tauri build

# Build macOS universal binary (Intel + Apple Silicon)
bun run tauri:build:universal

# Output (universal): src-tauri/target/universal-apple-darwin/release/bundle/
# Output (single-arch): src-tauri/target/release/bundle/
# macOS: .dmg and .app
# Windows: .msi and .exe
# Linux: .deb, .appimage
```

**Universal binary prerequisites:**

```bash
# Install both Rust targets (one-time setup)
rustup target add x86_64-apple-darwin    # Intel
rustup target add aarch64-apple-darwin   # Apple Silicon (likely already installed)
```

**Verify a universal binary:**

```bash
file src-tauri/target/universal-apple-darwin/release/bundle/macos/fewd.app/Contents/MacOS/fewd
# Expected: Mach-O universal binary with 2 architectures: [x86_64] [arm64]
```

**Distribute to another Mac:**

1. Build: `bun run tauri:build:universal`
2. The `.dmg` is at `src-tauri/target/universal-apple-darwin/release/bundle/dmg/`
3. Copy the `.dmg` to the other Mac (AirDrop, USB, shared folder, etc.)
4. On the other Mac, open the `.dmg` and drag the app to Applications
5. First launch: right-click → Open (to bypass Gatekeeper), or run `xattr -cr /Applications/fewd.app`

### Database Location

The database location is configurable via Settings → Database Location.

**Default (local) paths:**

- Dev: `~/Library/Application Support/com.fewd.dev/fewd.db`
- Production: `~/Library/Application Support/com.fewd/fewd.db`

**Config file** (always local, never synced):

- `~/Library/Application Support/com.fewd.dev/config.json`
- Contains `db_dir` — when set, the app uses `<db_dir>/fewd.db` instead of the default

**Shared database (iCloud):**

- Use Settings → Database Location → Change Location to point to a shared iCloud folder
- The app switches to DELETE journal mode (single-file, safe for sync) when using a custom location
- A `.fewd.lock` file is placed alongside the database to warn about concurrent access from other machines
- Avoid using the app on two computers at the exact same time

**Inspect Database:**

```bash
# Default location (macOS dev)
sqlite3 ~/Library/Application\ Support/com.fewd.dev/fewd.db

# Check current location from config
cat ~/Library/Application\ Support/com.fewd.dev/config.json

# Or use GUI tool like DB Browser for SQLite
```

## CI/CD (GitHub Actions)

### Workflows

**`.github/workflows/ci.yml`** - Runs on every push and PR:

- ✅ Rust: `cargo fmt --check`, `cargo clippy`, `cargo test`
- ✅ TypeScript: `dprint check`, `bun run lint`, `bun test`
- ✅ Typos: `typos --config .typos.toml`

**`.github/workflows/build.yml`** - Runs on tags (releases):

- Builds macOS universal binary (Intel + Apple Silicon), Windows, Linux
- Uploads artifacts to GitHub Release (draft)

### Running CI Locally

```bash
# Full CI check (what runs in GitHub)
./scripts/ci-check.sh

# Or manually:
cargo fmt --check && cargo clippy -- -D warnings && cargo test
dprint check && bun run lint && bun test
typos
```

## Common Tasks

### Add a New Entity

1. Create migration in `src-tauri/migration/src/`
1. Add to `migration/src/lib.rs`
1. Create entity in `src-tauri/src/entities/`
1. Create service in `src-tauri/src/services/`
1. Create DTOs and commands in `src-tauri/src/commands/`
1. Register commands in `src-tauri/src/main.rs`
1. Create TypeScript types in `src/types/`
1. Create hooks in `src/hooks/`
1. Create UI component in `src/components/`

### Add a New Dependency

**Rust:**

```bash
cd src-tauri
cargo add <crate-name>
```

**Frontend:**

```bash
bun add <package-name>
```

### Debug Tauri Commands

Add logging:

```rust
#[tauri::command]
pub async fn my_command(data: SomeDto) -> Result<Response, String> {
    eprintln!("my_command called with: {:?}", data);
    // ... rest of function
}
```

View logs:

- Dev mode: Check terminal running `bun run tauri dev`
- Production: macOS Console.app, filter by app name

### Update Database Schema

1. Create new migration
1. Run `bun run tauri dev` (auto-applies migration)
1. If migration fails, check logs and fix
1. Test rollback: manually run `.down()` and `.up()` again

## Troubleshooting

### Common Issues

**“command not found: tauri”**

```bash
cargo install tauri-cli --version "^2.0.0"
```

**SQLite locked errors**

- Stop dev server
- Delete database file
- Restart dev server (recreates DB)

**React not hot reloading**

- Restart dev server
- Clear Vite cache: `rm -rf node_modules/.vite`

**Rust compile errors after pulling**

```bash
cd src-tauri
cargo clean
cargo build
```

**TypeScript errors after pulling**

```bash
rm -rf node_modules bun.lockb
bun install
```

## macOS Specific

### First-Time Setup

```bash
# Install Tauri CLI
cargo install tauri-cli

# Install dprint (code formatter)
cargo install dprint

# Install typos
brew install typos-cli
```

### Building for Distribution

macOS requires code signing for distribution. For personal use, unsigned builds work fine.

**Build a universal binary (Intel + Apple Silicon):**

```bash
bun run tauri:build:universal
```

This produces a single `.app` and `.dmg` that runs natively on both Intel and Apple Silicon Macs. The output is in `src-tauri/target/universal-apple-darwin/release/bundle/`.

**To bypass Gatekeeper on unsigned app:**

```bash
# Right-click app → Open (instead of double-click)
# Or remove quarantine:
xattr -cr /path/to/fewd.app
```

## Key Patterns

### Backend: Service Layer Pattern

```rust
// Command delegates to service
commands::person::create_person() 
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

## Resources

- [Tauri Docs](https://tauri.app/)
- [SeaORM Docs](https://www.sea-ql.org/SeaORM/)
- [TanStack Query Docs](https://tanstack.com/query/latest)
- [Tailwind Docs](https://tailwindcss.com/docs)

## Questions?

Check:

1. `REQUIREMENTS.md` - What to build
1. `IMPLEMENTATION_PLAN.md` - How to build it
1. This file - How to maintain it
1. GitHub Issues - Known problems/features