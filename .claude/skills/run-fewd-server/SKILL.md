---
name: run-fewd-server
description: Build, run, and drive the fewd server — both the HTTP API (/api) and the MCP server (/mcp). Use when asked to run, start, boot, smoke-test, or poke fewd's backend, call an MCP tool, provision an MCP token, or reproduce an API/MCP request end-to-end.
---

# Running and driving the fewd server

One Axum binary serves both surfaces: `/api/*` (JSON, CORS-scoped, no auth) and
`/mcp` (Streamable HTTP JSON-RPC, bearer-token auth, no CORS). Drive both with
`.claude/skills/run-fewd-server/driver.mjs`, which boots a **disposable**
instance — its own SQLite file under `target/fewd-driver/` on a kernel-assigned
port — so nothing it does can touch the family's `data/fewd.db` or collide with
`just dev` on :3000.

All paths and commands are relative to the repo root. Run them from there:
`DATABASE_PATH` defaults to `./data/fewd.db` and resolves against the working
directory, so a stray `cd server` creates a second, silently diverging database.

## Prerequisites

Everything is already on this machine; verified versions:

```bash
cargo --version   # cargo 1.98.0
bun --version     # 1.4.0
just --version
```

Use `bun`/`bunx`, never `npm`/`npx`.

## Build

```bash
cargo build --bin fewd-server
```

The driver runs this itself on every boot, so a separate build step is only
needed when you want the compile errors on their own.

## Run (agent path)

```bash
# boot a throwaway instance, exercise API + MCP end to end, tear down
bun .claude/skills/run-fewd-server/driver.mjs smoke

# boot one and leave it running; prints base URL, db path, log path, MCP token
bun .claude/skills/run-fewd-server/driver.mjs up
bun .claude/skills/run-fewd-server/driver.mjs down
```

`smoke` exits non-zero on the first failed assertion. A passing run looks like:

```
▸ boot
  ✓ server up — http://localhost:53136
▸ HTTP API
  ✓ GET /api/version — 0.1.0 @ 2c236f8c4
  ✓ GET /api/people — 4 seeded
  ✓ POST /api/people — Driver Smoke
  ✓ POST /api/recipes — Smoke Test Chili (smoke-test-chili)
  ✓ POST /api/meals — Dinner 2026-09-14
  ✓ GET /api/shopping-list — 2 lines, beef scaled to 0.25 lb
▸ MCP
  ✓ unauthenticated /mcp rejected — 401
  ✓ token provisioned — for Alex
  ✓ initialize + initialized — 90becf17-…
  ✓ tools/list — 18: create_meal, create_recipe, …
  ✓ tools/call whoami — Hello, Alex. You are authenticated with fewd.
  ✓ tools/call search_recipes — found smoke-test-chili over the HTTP-created row
  ✓ prompts/list + resources/list — 1 prompts, 2 resources
  ✓ off-allowlist Host rejected — 403 (MCP_ALLOWED_HOSTS opts LAN names in)

SMOKE OK
```

### One-off calls against a running instance

After `up`, each of these reuses the recorded base URL, token, and MCP session:

```bash
# HTTP API
bun .claude/skills/run-fewd-server/driver.mjs api GET /api/version
bun .claude/skills/run-fewd-server/driver.mjs api GET '/api/meals?start_date=2026-09-01&end_date=2026-09-30'
bun .claude/skills/run-fewd-server/driver.mjs api POST /api/people \
  '{"name":"CLI Person","birthdate":"2000-05-05","dislikes":[],"favorites":[]}'

# MCP: raw JSON-RPC (handshakes on first use)
bun .claude/skills/run-fewd-server/driver.mjs mcp tools/list
bun .claude/skills/run-fewd-server/driver.mjs mcp prompts/list
bun .claude/skills/run-fewd-server/driver.mjs mcp resources/list

# MCP: tools/call shorthand — unwraps content[].text and parses embedded JSON
bun .claude/skills/run-fewd-server/driver.mjs tool whoami
bun .claude/skills/run-fewd-server/driver.mjs tool get_family_overview
bun .claude/skills/run-fewd-server/driver.mjs tool search_recipes '{"query":"chili"}'
bun .claude/skills/run-fewd-server/driver.mjs tool create_recipe \
  '{"name":"Driver Toast","source":"manual","servings":2,"instructions":"Toast the bread.",
    "ingredients":[{"name":"bread","amount":{"kind":"single","value":2},"unit":"slice"}],
    "tags":["breakfast"]}'
```

Cross-surface checks work the way you'd want: a recipe written through
`tool create_recipe` shows up in `api GET /api/recipes`.

Read a tool's exact input schema before guessing at arguments:

```bash
bun .claude/skills/run-fewd-server/driver.mjs mcp tools/list | python3 -c '
import sys,json
t=[x for x in json.load(sys.stdin)["tools"] if x["name"]=="create_recipe"][0]
print("required:", t["inputSchema"]["required"])'
```

Server output for the running instance lands in `target/fewd-driver/server.log`;
its database is `target/fewd-driver/fewd.db` (inspect with `sqlite3`).

## Direct invocation

Most recent PRs here add or change one MCP tool (`rate_recipe`, `favorite_recipe`,
`update_recipe`). Two levels below the full boot:

```bash
cargo test --workspace                    # 350 lib + 105 migration + integration, all green
cargo test -p fewd-server mcp::handler    # 87 tool-level unit tests
```

`RequestContext<RoleServer>` cannot be constructed in a unit test, so anything
that must observe the authenticated identity end-to-end goes through the HTTP
transport — either `server/tests/mcp_auth_plumbing_test.rs` or this driver.

## Run (human path)

```bash
just dev    # Axum on :3000 + Vite on :5173, proxying /api; Ctrl-C to stop
```

This binds the **real** `data/fewd.db`. Use it to look at the UI, not to test
backend changes. Verified: `:3000/api/version` and `:5173/api/version` both
answer 200 within ~2s of boot.

## Gotchas

- **`/mcp` answers with SSE, not JSON.** Every response body is `data: {...}`
  lines interleaved with keep-alive `data:`/`id:`/`retry:` frames, even for a
  plain POST. `curl … | jq` fails on all of them; strip the prefix first
  (`grep '^data: {' | sed 's/^data: //'`). The driver's `parseSse` does this.
- **Three calls before any tool call.** `initialize` → read the
  `mcp-session-id` **response header** → `notifications/initialized` (expect
  **202**) → `tools/call`. rmcp serves `initialize` from `get_info(&self)`,
  which never reads request extensions, so the authenticated identity only
  reaches tools after the notification. A session id the server doesn't know
  gets **404**, not 401.
- **A token exists only after you mint one.** `POST /api/people/{id}/mcp-token`
  returns the plaintext exactly once. It **requires a JSON body** — `-d '{}'`
  with `Content-Type: application/json`; without one it's **415**, by design
  (the empty struct forces a CORS preflight so an HTML form can't rotate
  someone's token). `DELETE` on the same path revokes (**204**).
- **The `Host` header is checked.** rmcp allows `localhost`/`127.0.0.1`/`::1`;
  anything else is **403** until `MCP_ALLOWED_HOSTS=dietpi.local,…` opts it in.
  An entry without a port matches any port.
- **The two surfaces disagree on two wire details.** HTTP tags ingredient
  amounts `{"type":"single"}`; MCP tags them `{"kind":"single"}` — copying a
  payload across gives `missing field kind`. HTTP addresses recipes by uuid
  `id`; MCP addresses them by `slug` (`parent_recipe_slug`, `get_recipe`).
- **A fresh database is not empty.** `seed_data::seed_if_empty` inserts Alex,
  Jordan, Sam, and Pat whenever the people table is empty, so a brand-new DB
  already has four people and four provisionable token targets.
- **`GET /api/meals` and `/api/shopping-list` require `start_date` and
  `end_date`**; without them it's a 400 on the query string, not an empty list.
- **`meal.servings` comes back as a JSON string**, not an array — it's a TEXT
  column serialized straight out. Parse it twice.
- **`meal_type` is Title Case and `order_index` is the planner slot**
  (Breakfast=0, Lunch=1, Dinner=2, Snack=3). A lowercase type or a mismatched
  slot stores fine and then never renders.
- **`PORT=0` gets you a kernel-assigned port**, which `main.rs` logs as
  `Server running on http://localhost:<port>`. That is how the driver avoids
  fighting `just dev` for :3000. The driver reads the port back out of that
  line, so it pins `fewd_server=info` on top of whatever `RUST_LOG` it
  inherits; it also clears `MCP_ALLOWED_HOSTS` for the child, since an
  inherited allowlist would let the smoke test's off-allowlist `Host` through.
- **Two servers happily share one SQLite file** (WAL mode) — a leaked instance
  does not announce itself with a lock error. If results look stale, run
  `down`, then `pgrep -fl 'debug/fewd-server'`.

## Troubleshooting

| Symptom                                                                              | Fix                                                                                                                                                                                                                                                     |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `driver: server never announced a port` with an empty log                            | A previous instance leaked: `bun …/driver.mjs down`, then `pkill -f 'target/debug/fewd-server'`, then retry. If the log has content but no port, the `Server running on http://localhost:<port>` line in `main.rs` moved and the scrape needs updating. |
| `server exited 101` with `Failed to initialize database: … "file is not a database"` | The scratch DB is corrupt. `rm -rf target/fewd-driver` and re-run.                                                                                                                                                                                      |
| `driver: no running instance`                                                        | `api`/`mcp`/`tool` need `up` first; `smoke` tears itself down.                                                                                                                                                                                          |
| `tools/call → 401: {"error":"invalid or revoked token"}`                             | The token in `state.json` was revoked, or the instance was rebooted onto a fresh DB. Re-run `up`.                                                                                                                                                       |
| `tools/call → 401: {"error":"missing Authorization: Bearer <mcp-token>"}`            | Hand-rolled request without the header. Mint one via `POST /api/people/{id}/mcp-token`.                                                                                                                                                                 |
| MCP call returns 404                                                                 | Stale `mcp-session-id`; the driver clears it and re-handshakes automatically. Hand-rolled clients must redo `initialize`.                                                                                                                               |
| `no JSON frame in MCP response`                                                      | The body was not SSE. The status code is in the message just above it — start there.                                                                                                                                                                    |
| `<tool>: missing field 'x'. Check the tool's input schema.`                          | Read the real schema with the `tools/list` snippet above; MCP inputs are not the HTTP DTOs.                                                                                                                                                             |
