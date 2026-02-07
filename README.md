# fewd

A family meal planning desktop app built with Tauri (Rust + React). Plan meals, manage recipes, and generate shopping lists.

## Quick Start

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (via rustup)
- [Bun](https://bun.sh/) (JavaScript runtime and package manager)

Optional (for code quality checks):

- dprint: `cargo install dprint`
- typos-cli: `brew install typos-cli`

### Setup and Run

```bash
# 1. Install JavaScript dependencies (includes Tauri CLI)
bun install

# 2. Run in development mode (compiles Rust + starts React dev server)
bun run tauri dev
```

The first run will take a few minutes to compile Rust dependencies. Subsequent runs are fast.

### Build for Production

```bash
bun run tauri build
```

Output will be in `src-tauri/target/release/bundle/`.

## Current Status

- [x] Project scaffolding (Tauri + React + SeaORM)
- [x] Family member management (create, edit, delete)
- [ ] Recipe management
- [ ] Meal planning calendar
- [ ] Shopping list generation

## Documentation

- **REQUIREMENTS.md** - Full specifications and data models
- **IMPLEMENTATION_PLAN.md** - Step-by-step build guide
- **CLAUDE.md** - Development guide for AI assistants

## Tech Stack

**Backend:** Rust + Tauri 2 + SeaORM + SQLite

**Frontend:** React 18 + TypeScript + Vite + TanStack Query + Tailwind CSS

## Development

### Commands

```bash
# Development
bun run tauri dev          # Run with hot reload

# Testing
cargo test                 # Rust tests (from src-tauri/)
bun run test               # Frontend tests

# Linting & Formatting
cargo fmt                  # Format Rust code (from src-tauri/)
cargo clippy               # Lint Rust code (from src-tauri/)
dprint fmt                 # Format frontend code
bun run lint               # Lint frontend code
typos                      # Check for typos

# All CI checks at once
./scripts/ci-check.sh
```

### Project Structure

```
fewd/
├── src/                   # React frontend
│   ├── components/        # UI components
│   ├── hooks/             # TanStack Query hooks
│   ├── types/             # TypeScript type definitions
│   └── App.tsx
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── commands/      # Tauri command handlers (thin layer)
│   │   ├── entities/      # SeaORM entities (DB models)
│   │   ├── services/      # Business logic
│   │   ├── db.rs          # Database initialization
│   │   └── main.rs
│   └── migration/         # SeaORM database migrations
├── .github/workflows/     # CI/CD
└── scripts/               # Development scripts
```

### Database

SQLite database is stored locally:

- **Dev:** `~/Library/Application Support/com.fewd.dev/fewd.db`
- **Production:** `~/Library/Application Support/com.fewd/fewd.db`

Inspect with: `sqlite3 ~/Library/Application\ Support/com.fewd.dev/fewd.db`

## License

MIT
