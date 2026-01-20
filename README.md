# fewd

A family meal planning desktop app built with Tauri (Rust + React). Plan meals, manage recipes, and generate shopping lists.

## Quick Start

```bash
# Install dependencies
bun install

# Run in development mode
bun run tauri dev

# Build for production
bun run tauri build
```

## Documentation

- **REQUIREMENTS.md** - Full specifications and data models
- **IMPLEMENTATION_PLAN.md** - Step-by-step build guide
- **CLAUDE.md** - Development guide for AI assistants
- **SETUP_SCRIPTS.md** - Linting/testing setup

## Features

- ✅ Manage family members with dietary preferences
- ✅ Store and organize recipes
- ✅ Import recipes from markdown
- ✅ Plan meals for the week
- ✅ Assign different recipes to different people
- ✅ Support ad-hoc food items (not just recipes)
- ✅ Generate shopping lists with unit conversion
- ✅ Desktop app (macOS, Windows, Linux)
- ✅ Local SQLite storage (no cloud required)

## Tech Stack

**Backend:**

- Rust + Tauri 2
- SeaORM + SQLite
- Unit conversion with `uom`

**Frontend:**

- React 18 + TypeScript
- Vite + TanStack Query
- Tailwind CSS

## Development

### Prerequisites

- Rust (via rustup)
- Bun.js (JavaScript runtime)
- Tauri CLI: `cargo install tauri-cli`
- dprint: `cargo install dprint` (optional, for formatting)
- typos-cli: `brew install typos-cli` (optional, for spell checking)

### Commands

```bash
# Development
bun run tauri dev          # Run with hot reload

# Testing
cargo test                 # Rust tests
bun test                   # Frontend tests
./scripts/ci-check.sh      # Run all CI checks locally

# Linting & Formatting
cargo fmt                  # Format Rust code
cargo clippy               # Lint Rust code
dprint fmt                 # Format frontend code
bun run lint               # Lint frontend code
typos                      # Check for typos

# Building
bun run tauri build        # Build production app
```

### Project Structure

```
fewd/
├── src/                   # React frontend
│   ├── components/
│   ├── hooks/
│   ├── types/
│   └── App.tsx
├── src-tauri/            # Rust backend
│   ├── src/
│   │   ├── commands/     # Tauri command handlers
│   │   ├── entities/     # SeaORM entities
│   │   ├── services/     # Business logic
│   │   └── main.rs
│   └── migration/        # Database migrations
├── .github/
│   └── workflows/        # CI/CD workflows
└── scripts/              # Development scripts
```

## Database

SQLite database location:

- **Development:** `~/Library/Application Support/com.fewd.dev/fewd.db`
- **Production:** `~/Library/Application Support/com.fewd/fewd.db`

Inspect with:

```bash
sqlite3 ~/Library/Application\ Support/com.fewd.dev/fewd.db
```

## CI/CD

GitHub Actions runs on every push:

- ✅ Rust formatting, linting, tests
- ✅ Frontend formatting (dprint), linting, tests
- ✅ Typo checking

Build workflow runs on git tags:

```bash
git tag v0.1.0
git push origin v0.1.0
```

Creates release with builds for macOS, Windows, Linux.

## Contributing

1. Read CLAUDE.md for code standards
1. Run `./scripts/ci-check.sh` before committing
1. Keep commits focused and descriptive
1. Update docs if changing behavior

## License

MIT
