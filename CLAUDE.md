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

**Typography:** Self-hosted variable fonts in `public/fonts/`:
- Headings: Playfair Display (serif) — configured as `fontFamily.heading` in Tailwind
- Body: DM Sans (sans-serif) — configured as `fontFamily.sans` override in Tailwind

**Design Tokens (`src/index.css` `@layer components`):**

| Token | Usage |
|-------|-------|
| `.btn` + `.btn-xs`/`.btn-sm`/`.btn-md` | Button sizes (include focus ring + transition) |
| `.btn-primary`/`.btn-secondary`/`.btn-outline`/`.btn-ghost`/`.btn-danger` | Button variants |
| `.input`/`.input-sm` | Text inputs, selects, textareas |
| `.card`/`.card-hover` | Content containers with shadow + rounded-xl |
| `.tag` | Rounded-full pills for labels |
| `.panel-primary`/`.panel-secondary`/`.panel-warning`/`.panel-error` | Colored card variants |

**Animation Utilities (`src/index.css` `@layer utilities`):**
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

**Color Palette** (in `tailwind.config.js`):
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

### Test Commands

```bash
# Rust tests
cargo test

# Frontend tests
bun test

# Frontend tests (watch mode)
bun run test:watch
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
bun run dev:server   # Axum backend on port 3001
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