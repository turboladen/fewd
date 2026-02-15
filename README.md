# fewd

Family meal planner. Plan weekly meals for each family member, manage recipes, generate shopping lists, and get AI-powered suggestions — all from a single self-hosted binary.

## Features

- **Family Members** — Track each person's dietary goals, favorites, and dislikes
- **Recipe Management** — Create recipes manually, import from markdown or URLs; tag, favorite, and search
- **Meal Planning** — Weekly calendar view with per-person meal assignment (recipe or ad-hoc items)
- **Shopping Lists** — Aggregated ingredient list for any week, with automatic unit conversion and source tracking
- **AI-Powered** — Recipe suggestions, adaptation, and extraction via Claude API
- **Meal Templates** — Save and reuse common meal combinations

## Architecture

- **Backend:** Rust (Axum + SeaORM + SQLite)
- **Frontend:** React 18 + TypeScript + Vite + TanStack Query + Tailwind
- **Deployment:** Single binary that embeds the SPA and serves everything over HTTP

The frontend is compiled by Vite into `dist/`, then embedded into the Rust binary at compile time via `rust-embed`. The result is one executable that serves the API (`/api/*`) and the web UI on a configurable port.

## Quick Start

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (via rustup)
- [Bun](https://bun.sh/) (JavaScript runtime and package manager)
- [just](https://github.com/casey/just) (command runner)

Optional (for code quality checks):

- dprint: `cargo install dprint`
- typos-cli: `brew install typos-cli`

### Development

```bash
bun install
just dev          # Runs Axum + Vite dev servers concurrently
```

The first run will take a few minutes to compile Rust dependencies. Subsequent runs are fast.

### Build for Production

```bash
just build        # Build frontend + release server binary
```

The binary lands at `server/target/release/fewd-server`.

### Run in Production

```bash
DATABASE_PATH=/path/to/fewd.db PORT=3000 ./fewd-server
```

| Variable        | Default          | Description                      |
| --------------- | ---------------- | -------------------------------- |
| `DATABASE_PATH` | `./data/fewd.db` | Path to the SQLite database file |
| `PORT`          | `3000`           | HTTP port to listen on           |

The database and its parent directory are created automatically on first run.

## Deploying to a Server

The app runs as a single binary with no external dependencies. Ideal for ARM64 devices like Raspberry Pi, ODroid, or DietPi.

### Prerequisites (macOS build machine)

```bash
rustup target add aarch64-unknown-linux-gnu
brew install messense/macos-cross-toolchains/aarch64-unknown-linux-gnu
```

### First-time setup

```bash
just setup-remote user@hostname
```

This SSHs into the target and creates a `fewd` system user, `/opt/fewd/` directories, and installs a systemd service that auto-starts on boot.

### Deploy

```bash
just deploy user@hostname
```

Cross-compiles for ARM64, copies the binary and service file, and restarts the service. Takes ~2 minutes.

### Verify

```bash
ssh user@hostname "systemctl status fewd"     # Should show active (running)
ssh user@hostname "journalctl -u fewd -n 20"  # View recent logs
```

Then open `http://hostname:3000` in a browser.

### Useful commands

```bash
ssh user@hostname "sudo systemctl stop fewd"       # Stop
ssh user@hostname "sudo systemctl restart fewd"     # Restart
ssh user@hostname "journalctl -u fewd -f"           # Live log tail
```

## Commands

```bash
# Development
just dev                   # Run with hot reload (server + client)

# Building
just build                 # Build frontend + server (release)
just build-arm64           # Cross-compile for Linux ARM64

# Deploying
just setup-remote user@host  # First-time server setup (creates user, dirs, systemd service)
just deploy user@host        # Build ARM64 + deploy + restart service

# Testing & Linting
just ci                    # Run all CI checks locally
```

## CI

Pushes to any branch run linting, formatting, and tests. Tagged releases (`v*`) build binaries for macOS (Intel + ARM), Linux x64, and Linux ARM64 — uploaded as draft GitHub Releases.

## Project Structure

```
fewd/
├── src/                   # React frontend
│   ├── components/        # UI components
│   ├── hooks/             # TanStack Query hooks
│   ├── types/             # TypeScript type definitions
│   └── App.tsx
├── server/                # Rust backend
│   ├── src/
│   │   ├── routes/        # Axum route handlers
│   │   ├── entities/      # SeaORM entities (DB models)
│   │   ├── services/      # Business logic
│   │   ├── db.rs          # Database initialization
│   │   └── main.rs
│   └── migration/         # SeaORM database migrations
├── deploy/                # Systemd service + setup script
├── .github/workflows/     # CI/CD
└── Justfile               # Development commands
```

## Documentation

- **REQUIREMENTS.md** — Full specifications and data models
- **IMPLEMENTATION_PLAN.md** — Build guide for upcoming features
- **CLAUDE.md** — Development guide for AI assistants

## License

MIT
