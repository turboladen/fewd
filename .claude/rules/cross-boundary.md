---
paths:
  - "server/**"
  - "src/**"
---

# Cross-boundary conventions

Invariants the type system does not enforce but production code assumes. Break one and the data lands in the database while the UI silently fails to render it.

## Meal type and slot encoding

`Meal.meal_type` is the `MealType` enum (`server/src/entities/sea_orm_active_enums.rs`), stored and serialized as Title Case: `Breakfast`, `Lunch`, `Dinner`, `Snack`. `MealType::from_str` accepts any casing, so write paths should parse through it. The web planner compares with strict equality (`meal.meal_type === 'Dinner'`).

`Meal.order_index` is a slot number, not a sort key. `DEFAULT_MEALS` in `src/components/MealPlanner.tsx` renders Breakfast=0, Lunch=1, Dinner=2, and a meal appears only when both its type and its index match a slot. Snack has no slot in the planner. On the MCP boundary, `default_order_index` in `server/src/mcp/schemas/meals.rs` assigns the slot; any other write path (HTTP routes, future tools, SQL migrations) must assign the same index, or the meal will not render.

## CSRF protection on state-changing POST routes

State-changing POST routes (rotation, provisioning, anything that mutates server state without a JSON body in the normal client flow) must take a `Json<T>` body extractor, even an empty `#[derive(Deserialize)] struct Empty {}`. HTML form posts are CORS-simple and bypass preflight without a body type; requiring `Content-Type: application/json` forces preflight, which the locked-down CORS allowlist then rejects from non-allowed origins. DELETE is non-simple by method, so it is already preflighted. See `routes::people::provision_mcp_token`.

## MCP wire shapes are not the HTTP DTOs

Ingredient amounts are tagged `{"type":"single"}` over `/api` and `{"kind":"single"}` over `/mcp`; recipes are addressed by uuid `id` over HTTP and by `slug` over MCP. Read a tool's schema from `tools/list` rather than reusing a DTO.

## Database path

The server reads `DATABASE_PATH`, defaulting to `./data/fewd.db` relative to its working directory. `just dev` runs `cargo run --bin fewd-server` from the workspace root, so the dev database is `data/fewd.db`; the deployed unit sets `/opt/fewd/data/fewd.db`. Never run the server from `server/`: that creates a parallel `server/data/fewd.db` that silently drifts from the one the UI reads. `bun run dev:server`, `bun run dev:full`, and `just db-reset` currently do exactly that, so use `just dev`.
