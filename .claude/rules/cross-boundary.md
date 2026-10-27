---
paths:
  - "server/**"
  - "src/**"
---

# Cross-boundary conventions

Invariants the type system does not enforce but production code assumes. Break one and the data lands in the database while the UI silently fails to render it.

## Meal type and slot encoding

`Meal.meal_type` is the `MealType` enum (`server/src/entities/sea_orm_active_enums.rs`), stored and serialized as Title Case: `Breakfast`, `Lunch`, `Dinner`, `Snack`. `MealType::from_str` accepts any casing, so write paths should parse through it. The web planner compares with strict equality (`meal.meal_type === 'Dinner'`).

`Meal.order_index` is a slot number, not a sort key. `DEFAULT_MEALS` in `src/components/MealPlanner.tsx` defines the default slots Breakfast=0, Lunch=1, Dinner=2, and a meal with an index from 0 to 2 renders only when its type matches that slot: a Dinner stored at index 0 is invisible. Any meal with `order_index >= 3` renders as a custom meal whatever its type, which is where a Snack lands. On the MCP boundary, `default_order_index` in `server/src/mcp/schemas/meals.rs` assigns the index (Snack=3); any other write path (HTTP routes, future tools, SQL migrations) must assign the same mapping, or a default-slot meal will not render.

## CSRF protection on `/api` POST routes

Every POST under `/api` must be a JSON request (`Content-Type: application/json`, parameters and case ignored) or carry an `X-Requested-With` header. `routes::csrf::reject_cors_simple_posts` answers 403 to anything else. `api_routes()` wraps everything the private `routes()` registers in that guard, so a new POST route, with or without a body, needs no guard of its own. A router mounted outside `api_routes()` gets no guard.

The reason is that an HTML form, `fetch` or `sendBeacon` can POST cross-site without a preflight, but only with a form, multipart or text/plain body, or none, and never with a custom header. A JSON or `X-Requested-With` request forces a preflight, and the server grants no CORS at all, so that preflight fails from every other origin. The guard plus the absence of any CORS grant is the invariant: `server/src/main.rs` has no CORS layer on purpose, because the SPA and the Vite dev proxy call `/api` same-origin. Adding a CORS layer, even with an origin allowlist, weakens it.

`src/lib/api.ts` sends `X-Requested-With` on every request, which is what lets a bodiless or multipart POST through. PUT and DELETE are preflighted by method, so the guard leaves them alone.

## MCP wire shapes are not the HTTP DTOs

Ingredient amounts are tagged `{"type":"single"}` over `/api` and `{"kind":"single"}` over `/mcp`; recipes are addressed by uuid `id` over HTTP and by `slug` over MCP. Read a tool's schema from `tools/list` rather than reusing a DTO.

## Database path

The server reads `DATABASE_PATH`, defaulting to `./data/fewd.db` relative to its working directory. `just dev`, `just db-reset`, `bun run dev:server`, and `bun run dev:full` all run the server from the workspace root, so the dev database is `data/fewd.db`; the deployed unit sets `/opt/fewd/data/fewd.db`. Never run the server from `server/`: that creates a parallel `server/data/fewd.db` that silently drifts from the one the UI reads.
