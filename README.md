# fewd

Family meal planner. Plan weekly meals for each family member, manage recipes, generate shopping lists, and get AI-powered suggestions — all from a single self-hosted binary.

## Features

- **Family Members** — Track each person's dietary goals, favorites, and dislikes
- **Recipe Management** — Create recipes manually, import from markdown or URLs; tag, favorite, and search
- **Meal Planning** — Weekly calendar view with per-person meal assignment (recipe or ad-hoc items)
- **Shopping Lists** — Aggregated ingredient list for any week, with automatic unit conversion and source tracking
- **AI-Powered** — Recipe suggestions, adaptation, and extraction via Claude API
- **Meal Templates** — Save and reuse common meal combinations
- **MCP Server** — Plan meals and generate shopping lists from Claude Desktop, Claude.ai connectors, or Claude Code (see [MCP Server](#mcp-server) below)

## Architecture

- **Backend:** Rust (Axum + SeaORM + SQLite)
- **Frontend:** React 18 + TypeScript + Vite + TanStack Query + Tailwind
- **Deployment:** Single binary that embeds the SPA and serves everything over HTTP

The frontend is compiled by Vite into `dist/`, then embedded into the Rust binary at compile time via `rust-embed`. The result is one executable that serves the JSON API (`/api/*`), the MCP server (`/mcp`), and the web UI on a configurable port.

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

## MCP Server

fewd exposes its recipe and meal-planning domain via a [Model Context Protocol](https://modelcontextprotocol.io) server so you can drive meal planning from a Claude client. The canonical flow: tell Claude your family's schedule for the week, have it pick or invent recipes, schedule them as dinners, and spit out a shopping list you can check off.

The MCP endpoint is mounted at `/mcp` on the same port as the web UI. Transport is Streamable HTTP (no separate process or binary to run).

### Tools

| Tool                                           | Purpose                                                                                       |
| ---------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `list_recipes`, `search_recipes`, `get_recipe` | Discover existing recipes by slug or name substring                                           |
| `list_people`                                  | Active family members with dietary goals, dislikes, favorites                                 |
| `get_family_overview`                          | Markdown summary of all active family members in one block (tool mirror of the resource)      |
| `list_meals(start_date, end_date)`             | Meals already scheduled in a range                                                            |
| `get_shopping_list(start_date, end_date)`      | Aggregated ingredient list with unit conversion                                               |
| `create_recipe(...)`                           | Add a new recipe. Slug is auto-generated from the name                                        |
| `create_meal(...)`                             | Schedule a meal — assigns people (by name) to a recipe (by slug) or an ad-hoc ingredient list |
| `whoami`                                       | Returns the authenticated family member's name. Useful for verifying your client config       |

### Resource

- `fewd://family/overview` — Markdown summary of every active family member. Clients that auto-load MCP resources will pick this up at conversation start. Mirrored by the `get_family_overview` tool above for clients (e.g. Claude Desktop) that surface resources for user attachment but don't let the LLM fetch them autonomously.

### Authentication

fewd uses a deliberately light "family-name bearer" scheme to stay out of your way on your LAN. Every MCP request must send:

```
Authorization: Bearer <family-member-name>
```

The name is matched case-insensitively against active `Person` rows. Unknown names get a `401`. There's no OAuth, no API keys — the assumption is that only people on your local network can reach fewd.

### Enable in Claude Desktop

Claude Desktop's config file only supports stdio MCP servers directly, so we bridge fewd's HTTP endpoint through [`mcp-remote`](https://github.com/geelen/mcp-remote) — a small npm package that runs as a stdio server and forwards requests to the remote URL.

**Prerequisite:** Bun (the project's runtime) — `bunx` is the launcher. If you've run `just dev` you already have it. (If you'd rather use Node, swap `bunx` for `npx` and install Node with `brew install node`.)

Claude Desktop's settings → Developer → Edit Config opens `claude_desktop_config.json`; add this entry:

```json
{
  "mcpServers": {
    "fewd": {
      "command": "bunx",
      "args": [
        "mcp-remote",
        "http://<fewd-host>:3000/mcp",
        "--header",
        "Authorization:${FEWD_BEARER}",
        "--transport",
        "http-only"
      ],
      "env": {
        "FEWD_BEARER": "Bearer Alice"
      }
    }
  }
}
```

- Replace `<fewd-host>` with the hostname (or IP) of whatever machine is running `fewd-server` — usually the same Raspberry Pi / home server you configured via `just setup-remote`, or `localhost` if you're running it on the same machine as Claude Desktop.
- Replace `Alice` with an active family member's name.
- The `--header "Authorization:${FEWD_BEARER}"` + `env.FEWD_BEARER` split is intentional: it dodges a Windows-specific quoting bug in the launchers where spaces inside `args` get mangled ([upstream note](https://github.com/geelen/mcp-remote#custom-headers-authentication)).
- `--transport http-only` pins the bridge to Streamable HTTP. Without it, `mcp-remote` tries the deprecated HTTP+SSE transport as a fallback — fewd only speaks Streamable HTTP.

Fully quit and relaunch Claude Desktop. You should see fewd's tools in the MCP indicator. Call `whoami` first to confirm the bearer resolves correctly.

**Troubleshooting `Failed to spawn process: No such file or directory`** — Claude Desktop couldn't find `bunx` in its PATH. Run `which bunx` in your terminal; if it's somewhere Claude Desktop isn't searching, replace `"command": "bunx"` with the absolute path (e.g. `"command": "/Users/you/.bun/bin/bunx"`).

### Scope of v1

Currently exposed: recipes + people (read) and recipes + meals (write). **Not** exposed: cocktails / bar inventory, meal templates, updates, deletes, AI enhancement endpoints (the web UI still uses those). The MCP server is intended for meal-planning conversations, not administration.

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
│   │   ├── mcp/           # MCP server (tools + resources over Streamable HTTP)
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
