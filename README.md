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

| Variable            | Default          | Description                                                                                                                                                         |
| ------------------- | ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `DATABASE_PATH`     | `./data/fewd.db` | Path to the SQLite database file                                                                                                                                    |
| `PORT`              | `3000`           | HTTP port to listen on                                                                                                                                              |
| `MCP_ALLOWED_HOSTS` | _(unset)_        | Comma-separated extra hostnames allowed in the MCP `Host` header — set this when reaching `/mcp` from anywhere other than localhost. See [MCP Server](#mcp-server). |

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

| Tool                                                   | Purpose                                                                                                                                                                                                                                                                                            |
| ------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list_curated_recipes`, `search_recipes`, `get_recipe` | Discover existing recipes. `list_curated_recipes` returns a ≤30-row favorites/recent/top-rated shortlist; `search_recipes` requires at least one filter (query, tags, max_total_time_minutes, min_rating, is_favorite, unplanned_since_days, excludes_for_persons, includes_ingredient_substrings) |
| `list_diet_tags`                                       | Canonical diet-tag vocabulary (tag + meaning). Translate a person's free-form `dietary_goals` into these, then filter `search_recipes` by `tags`. Mirrors the `fewd://diet-tags` resource. See [Diet tags](#diet-tags)                                                                             |
| `list_people`                                          | Active family members with dietary goals, dislikes, favorites                                                                                                                                                                                                                                      |
| `get_family_overview`                                  | Markdown summary of all active family members in one block (tool mirror of the resource)                                                                                                                                                                                                           |
| `list_meals(start_date, end_date)`                     | Meals already scheduled in a range                                                                                                                                                                                                                                                                 |
| `get_shopping_list(start_date, end_date)`              | Aggregated ingredient list with unit conversion                                                                                                                                                                                                                                                    |
| `create_recipe(...)`                                   | Add a new recipe. Slug is auto-generated from the name. Apply applicable diet tags (see `list_diet_tags`) so the recipe is discoverable by dietary goal                                                                                                                                            |
| `update_recipe(...)`                                   | Edit an existing recipe, identified by slug. Only the fields sent are written; `ingredients` / `tags` / `instructions` replace whole rather than merge, and renaming does not change the slug                                                                                                      |
| `favorite_recipe(slug, is_favorite)`                   | Mark a recipe as a family favorite, or unmark it. `is_favorite` is set absolutely rather than toggled, so repeating a call leaves the recipe in the same state. Drives the `list_curated_recipes` shortlist and `search_recipes`'s `is_favorite` filter                                            |
| `rate_recipe(slug, rating)`                            | Rate a recipe 1-5 stars. Whole stars only — a fractional value rounds to the nearest, and a value that does not round into 1-5 is rejected. The returned row carries the stored value. Feeds `search_recipes`'s `min_rating` filter                                                                |
| `unrate_recipe(slug)`                                  | Remove a recipe's star rating. Distinct from rating 1 star: `min_rating` excludes unrated recipes entirely, so a cleared recipe leaves rating-filtered searches rather than ranking last                                                                                                           |
| `create_meal(...)`                                     | Schedule a meal — assigns people (by name) to a recipe (by slug) or an ad-hoc ingredient list                                                                                                                                                                                                      |
| `whoami`                                               | Returns the authenticated family member's name. Useful for verifying your client config                                                                                                                                                                                                            |

### Resources

- `fewd://family/overview` — Markdown summary of every active family member. Clients that auto-load MCP resources will pick this up at conversation start. Mirrored by the `get_family_overview` tool above for clients (e.g. Claude Desktop) that surface resources for user attachment but don't let the LLM fetch them autonomously.
- `fewd://diet-tags` — Markdown list of the canonical diet-tag vocabulary. Mirrored by the `list_diet_tags` tool above.

### Diet tags

`Person.dietary_goals` is free-form text ("low-carb", "pescatarian", "trying to eat more veggies") and recipe `tags` are free-form too, so there's no structured diet field to match against. Instead, fewd publishes a **conventional diet-tag vocabulary** that the LLM translates free-form goals into, then filters `search_recipes` by `tags`. The tool's own `list_diet_tags` description (and the `fewd://diet-tags` resource) are the machine-discoverable source of truth; this table is the human mirror.

| Tag             | Meaning                                                                             |
| --------------- | ----------------------------------------------------------------------------------- |
| `vegetarian`    | No meat, poultry, or seafood. May include dairy and eggs.                           |
| `vegan`         | No animal products at all — no meat, dairy, eggs, or honey.                         |
| `pescatarian`   | No meat or poultry, but seafood is allowed.                                         |
| `gluten-free`   | Contains no wheat, barley, rye, or other gluten sources.                            |
| `dairy-free`    | Contains no milk, cheese, butter, or other dairy.                                   |
| `nut-free`      | Contains no tree nuts or peanuts.                                                   |
| `low-carb`      | Low in carbohydrates; minimizes grains, sugars, and starches.                       |
| `keto`          | Very low carb, high fat — suitable for a ketogenic diet.                            |
| `paleo`         | No grains, legumes, dairy, or refined sugar (paleo template).                       |
| `low-sodium`    | Prepared with little added salt; suitable for sodium-restricted diets.              |
| `high-protein`  | Notably high protein per serving; suits muscle-gain or satiety goals.               |
| `whole30`       | No added sugar, grains, dairy, legumes, or alcohol (Whole30 elimination template).  |
| `mediterranean` | Emphasizes vegetables, whole grains, fish, and olive oil; minimal red meat.         |
| `low-fodmap`    | Low in fermentable carbs (FODMAPs) that can trigger IBS symptoms.                   |
| `halal`         | Permissible under Islamic dietary law (no pork or alcohol; meat slaughtered halal). |
| `kosher`        | Conforms to Jewish dietary law (no pork or shellfish; meat and dairy not mixed).    |

Enforcement is **soft**: `create_recipe` accepts any tags, but its description encourages applying these when a recipe qualifies. Recipes authored before this convention won't carry diet tags until re-tagged — apply tags on new recipes going forward, and re-tag existing ones with `update_recipe` (or in the Recipes tab of the web UI), for the catalog to narrow well by diet. Note that `update_recipe`'s `tags` replaces the whole list, so send the recipe's existing tags alongside the new ones.

### Authentication and threat model

fewd uses a per-person opaque-token bearer scheme. Every MCP request must send:

```
Authorization: Bearer <mcp-token>
```

Tokens are 256-bit random secrets, base64url-encoded for transport. They're hashed at rest with argon2id and compared in constant time. Unknown or revoked tokens get a `401`. The plaintext is shown to you exactly once at provision time and never stored — the server keeps only the hash plus an 8-character fingerprint for UI identification.

**Provision a token.** Open the web UI Settings panel, find the family member you want to give MCP access to, and click _Provision token_. Copy the displayed string into your client config (see _Enable in Claude Desktop_ below) before closing the dialog — there is no recovery path. Re-provisioning rotates the token; the previous one stops working immediately.

**Revoke a token.** From the same Settings row, _Revoke_ nulls out the hash and fingerprint. Any client still presenting the old plaintext gets a `401` on its next call.

**Be explicit about what this is and isn't:**

- **Tokens are real credentials.** Treat the plaintext like a password — paste it into a config file, don't share it in chat, and don't commit it to a repo.
- **`/mcp` LAN-only is now a defense-in-depth recommendation, not a hard requirement.** The token verification is independently strong, but the server still binds `0.0.0.0` with no rate-limiting, no audit trail, and no telemetry on revocation events. **For internet-facing exposure, put a fronting proxy with mTLS or an additional auth layer in front of `/mcp`.** Tokens alone aren't a substitute for hardening at the deployment layer.
- **Any authenticated family member can read and write anything.** There is no per-user authorization. Anyone with a valid token can read every recipe, schedule meals on anyone's behalf, and create new recipes. Audit logging at the database layer is not in place — writes aren't attributed to the authenticator. Issue `fewd-2y6.8` (typed `AuthenticatedPerson` extractor that forces every write tool to consult the authenticator at compile time) tracks the next step.

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
        "http-only",
        "--allow-http"
      ],
      "env": {
        "FEWD_BEARER": "Bearer <paste-the-token-from-Settings-here>"
      }
    }
  }
}
```

- Replace `<fewd-host>` with the hostname (or IP) of whatever machine is running `fewd-server` — usually the same Raspberry Pi / home server you configured via `just setup-remote`, or `localhost` if you're running it on the same machine as Claude Desktop.
- Replace `<paste-the-token-from-Settings-here>` with the plaintext shown when you provisioned the token in the web Settings panel. (Lost it? Re-provision — the old one is gone.)
- The `--header "Authorization:${FEWD_BEARER}"` + `env.FEWD_BEARER` split is intentional: it dodges a Windows-specific quoting bug in the launchers where spaces inside `args` get mangled ([upstream note](https://github.com/geelen/mcp-remote#custom-headers-authentication)).
- `--transport http-only` pins the bridge to Streamable HTTP. Without it, `mcp-remote` tries the deprecated HTTP+SSE transport as a fallback — fewd only speaks Streamable HTTP.
- `--allow-http` is required because the URL is plain `http://` (fewd serves the MCP endpoint over HTTP, not HTTPS). `mcp-remote` refuses non-`https` URLs unless the host is `localhost`/`127.0.0.1`; for any other host (`dietpi.local`, a LAN IP, …) you'll get a connection failure without this flag.

Fully quit and relaunch Claude Desktop. You should see fewd's tools in the MCP indicator. Call `whoami` first to confirm the bearer resolves correctly.

**Troubleshooting `Failed to spawn process: No such file or directory`** — Claude Desktop couldn't find `bunx` in its PATH. Run `which bunx` in your terminal; if it's somewhere Claude Desktop isn't searching, replace `"command": "bunx"` with the absolute path (e.g. `"command": "/Users/you/.bun/bin/bunx"`).

**Troubleshooting `Forbidden: Host header is not allowed` (HTTP 403)** — the MCP transport ships with DNS-rebinding protection that allowlists the `Host` header, defaulting to `localhost`/`127.0.0.1`/`::1` only. When Claude Desktop reaches the server at, say, `http://dietpi.local:3000/mcp`, the `Host` header is `dietpi.local:3000` and the server rejects it. Set `MCP_ALLOWED_HOSTS` to whatever address the client uses — that may be an mDNS/DNS name (`dietpi.local`, `homeserver`) **or** a raw IP literal (`192.168.1.42`, `[fe80::1]`). For systemd deployments that's an `Environment=` line in `deploy/fewd.service`. Multiple entries go comma-separated; a bare entry matches any port, `entry:port` matches one port.

### Weekly dinner-planning skill (Claude Desktop)

fewd ships a companion [Agent Skill](https://www.anthropic.com/news/skills) in `skills/weekly-dinner-plan/` that runs the household's canonical weekly dinner-planning workflow on top of the MCP tools above. It mirrors the server's `weekly_dinner_plan` MCP prompt — useful because Claude Desktop advertises MCP prompts but doesn't reliably surface them in its UI — and additionally prompts you for any planning detail you left out (ingredients to use up, seasonal style, effort limits, …).

**Import it into Claude Desktop:**

1. Enable the fewd MCP connector first (see _Enable in Claude Desktop_ above) — the skill calls fewd's tools, so the connector has to be connected.
2. In Claude's **Settings → Capabilities**, turn on **Skills**. They run in the code-execution sandbox, so that capability must be enabled too; on Team/Enterprise plans an admin has to enable Skills org-wide first.
3. Add `skills/weekly-dinner-plan/` as a custom skill — zip the folder (with `SKILL.md` at its root) and upload it from the Skills settings. Skills are account-level and shared across Claude.ai, Desktop, and Code. The exact upload affordance moves between versions, so see Anthropic's [Agent Skills docs](https://www.anthropic.com/news/skills) for the current steps.

Then just ask something like "plan dinners for this week" — the skill triggers on weekly meal-planning requests and runs the propose → confirm → schedule → shopping-list → fridge-printable flow, asking about any missing planning detail before it commits anything.

### Inspect and test with MCP Inspector

For local development — exploring tool surfaces, eyeballing JSON-RPC frames, sanity-checking a change before opening a PR — the official [MCP Inspector](https://github.com/modelcontextprotocol/inspector) is the right tool. It's a hosted web UI that speaks Streamable HTTP and lets you call tools by hand. **It is not a substitute for the automated test suite** (see `server/tests/service_tests.rs` and `server/tests/mcp_auth_plumbing_test.rs`); reach for it when you want to poke at a running server or diagnose a failing test, not to verify a PR.

**Setup:**

1. Provision a token for any active family member in the web UI (Settings → _Provision token_). Copy the plaintext.
2. In a fresh terminal: `bunx @modelcontextprotocol/inspector` — the UI boots at `http://localhost:6274`.
3. In the Inspector UI, set:
   - **Transport type:** Streamable HTTP
   - **URL:** `http://localhost:3000/mcp` (matches `just dev`)
   - **Authentication:** Bearer Token → paste the plaintext from step 1
4. Click **Connect**. The tools list populates from the server.

From there: pick a tool, fill the params pane, click **Run**, inspect the response. The Inspector's History panel shows every JSON-RPC frame on the wire — handy when comparing against `mcp_auth_plumbing_test.rs`.

See [`docs/mcp-testing.md`](docs/mcp-testing.md) for a catalog of copy-pasteable tool+params combos covering happy paths, error paths, and edge cases for every tool.

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
- **docs/mcp-testing.md** — Copy-pasteable tool+params combos for exploratory MCP testing via Inspector

## License

MIT
