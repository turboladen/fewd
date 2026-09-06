# MCP Testing Scenarios

Copy-pasteable tool + params combos for exploratory testing of the fewd MCP server via the [MCP Inspector](../README.md#inspect-and-test-with-mcp-inspector).

**This is not a substitute for the automated test suite.** Service-layer correctness lives in `server/tests/service_tests.rs`; transport plumbing lives in `server/tests/mcp_auth_plumbing_test.rs`. Use this doc to:

- Sanity-check tool behavior against a running server before opening a PR
- Diagnose a failing automated test by reproducing the call by hand
- Sketch a new tool's UX during development

If you find yourself running the same scenario manually more than twice, promote it to an automated test.

## Prerequisites

- `just dev` is running (server at `http://localhost:3000`).
- A token has been provisioned for an active family member (Settings → _Provision token_).
- Inspector is connected: Streamable HTTP, `http://localhost:3000/mcp`, Bearer = the token.
- Some recipes / people / meals exist in the DB so list-shaped tools return data. The seed sample family in `bun run dev:full` gives you a working baseline.

## Tool catalog

### `whoami`

Smoke test the auth chain.

**Happy path:**

```json
{}
```

Expect: `"Hello, <name>. You are authenticated with fewd."` — confirms the bearer middleware resolved the token to the right person.

**Failure mode to watch for:** if the response says any name other than the family member you provisioned the token for, the token-resolution path is broken — file a bug.

---

### `list_curated_recipes`

Bounded shortlist (≤30 unless the family has more than 30 favorites — favorites are never truncated).

**Happy path:**

```json
{}
```

Expect: a non-empty JSON array of `RecipeBrief` objects (slug, name, tags, total_time, etc.). The shape is favorites first, then most-recently-made, then top-rated, deduped.

**Edge case — over-cap rejection:** mark 501+ recipes as favorite (only worth doing if you're stress-testing). Expect a tool-level error citing the 500-row cap and the "mark fewer favorites" hint.

---

### `search_recipes`

The richest filter surface — most useful tool to exercise during development.

**Happy path — query substring:**

```json
{ "query": "chicken" }
```

Expect: recipes with "chicken" anywhere in the name (case-insensitive).

**Happy path — tags AND:**

```json
{ "tags": ["dinner", "easy"] }
```

Expect: only recipes tagged with both "dinner" AND "easy" (case-insensitive exact match per tag).

**Happy path — include substring (fewd-e7o):**

```json
{ "includes_ingredient_substrings": ["chicken"] }
```

Expect: recipes where some ingredient name contains "chicken." Try `"olive"` against your real catalog — should hit both "olive oil" and "olive" entries.

**Happy path — multi-substring AND:**

```json
{ "includes_ingredient_substrings": ["spam", "cheese"] }
```

Expect: only recipes with BOTH spam AND cheese as ingredients (possibly different ingredients per substring). If you have no such recipe, expect `[]` (empty array, not an error — see edge case below).

**Happy path — exclude by person dislikes:**

```json
{ "excludes_for_persons": ["Alice"], "tags": ["dinner"] }
```

Expect: dinner recipes that don't contain any of Alice's disliked ingredients. Names are case-insensitive.

**Happy path — composite filter:**

```json
{
  "tags": ["dinner"],
  "max_total_time_minutes": 30,
  "min_rating": 4.0,
  "includes_ingredient_substrings": ["chicken"]
}
```

Expect: ≤30-min dinner recipes rated ≥4 stars that include chicken in some ingredient. All filters AND together.

**Edge case — no-match returns empty, not error:**

```json
{ "includes_ingredient_substrings": ["zzzzzzz"] }
```

Expect: `[]` — empty array, success. Pinned by `search_filtered_includes_no_match_returns_empty_not_error`.

**Error path — bare call:**

```json
{}
```

Expect: tool-level error listing every accepted filter. The error message is the LLM's only path to discovering it needs to add a filter, so the wording matters.

**Error path — unknown person:**

```json
{ "excludes_for_persons": ["NotARealName"] }
```

Expect: tool-level error mentioning the unknown name AND pointing at `list_people`.

**Error path — wildcard-only query:**

```json
{ "query": "*" }
```

Expect: same bare-call error — `*` is normalized to "no query."

---

### `get_recipe`

Full record lookup by slug.

**Happy path:**

```json
{ "slug": "chicken-pot-pie" }
```

Expect: a `RecipeFull` object with ingredients, instructions, nutrition, parent slug if any.

**Error path — unknown slug:**

```json
{ "slug": "this-recipe-does-not-exist" }
```

Expect: tool-level error mentioning the missing slug AND pointing at `list_curated_recipes` / `search_recipes` for discovery.

**Error path — empty slug:**

```json
{ "slug": "   " }
```

Expect: tool-level error saying `slug` must not be empty or whitespace-only, NOT "no recipe with slug ''".

**Edge case — slug with whitespace / mixed case:**

```json
{ "slug": "  CHICKEN-Pot-Pie  " }
```

Expect: same result as the trimmed-lowercase form. The handler normalizes before lookup.

---

### `list_people`

**Happy path:**

```json
{}
```

Expect: array of active family members with dietary_goals, dislikes, favorites, notes, drink_preferences. The `mcp_token_hash` field MUST NOT appear — that's pinned by `mcp_token_service::tests::person_serialization_omits_mcp_token_hash`.

---

### `get_family_overview`

Markdown summary equivalent to the `fewd://family/overview` resource.

**Happy path:**

```json
{}
```

Expect: a markdown block (not JSON) summarizing every active family member's diet/dislikes/favorites/notes. Use this when planning meals — it's typically cheaper than `list_people` + per-person reasoning.

---

### `list_meals`

Date-range query.

**Happy path:**

```json
{ "start_date": "2026-05-01", "end_date": "2026-05-07" }
```

Expect: meals scheduled in that window, with assigned servings (who's eating which recipe and how many portions).

**Error path — invalid date format:**

```json
{ "start_date": "May 1, 2026", "end_date": "2026-05-07" }
```

Expect: tool-level error mentioning the offending field, the value, and the YYYY-MM-DD format.

**Error path — reversed range:**

```json
{ "start_date": "2026-05-07", "end_date": "2026-05-01" }
```

Expect: tool-level error explicitly calling out start > end. Without this, the service-layer SQL would silently return `[]` (indistinguishable from "no meals scheduled").

**Error path — over-wide range:**

```json
{ "start_date": "0001-01-01", "end_date": "9999-12-31" }
```

Expect: tool-level error citing the 366-day cap and the "Narrow" hint.

---

### `get_shopping_list`

Aggregated grocery list across meals in a date range.

**Happy path:**

```json
{ "start_date": "2026-05-01", "end_date": "2026-05-07" }
```

Expect: array of consolidated `ShoppingListItem` entries (ingredient name, total amount, unit, per-meal source list). Unit conversion happens server-side where compatible.

Same date-validation error paths as `list_meals`.

---

### `create_recipe`

State-changing. Be aware that calls land in the live DB — clean up afterward or use a throwaway dev DB.

**Happy path:**

```json
{
  "name": "Test Recipe",
  "source": "manual",
  "servings": 4,
  "instructions": "Stir.",
  "ingredients": [
    {
      "name": "test ingredient",
      "amount": { "kind": "single", "value": 1.0 },
      "unit": "cup"
    }
  ],
  "tags": ["test"]
}
```

Expect: the full created recipe with its auto-generated slug (probably `test-recipe`). Ingredient `amount` is a tagged union — use `{"kind": "single", "value": N}` for an exact amount or `{"kind": "range", "min": N, "max": M}` for a range.

**Error path — missing required field:**

```json
{
  "name": "Test Recipe",
  "servings": 4,
  "instructions": "Stir.",
  "ingredients": []
}
```

Expect: tool-level error naming the missing `source` field. Pinned by `lenient_parameters_extract_missing_required_field_captures_serde_error` (the contract that deserialize errors surface as tool-level errors rather than JSON-RPC protocol errors that get hidden by most MCP clients).

**Error path — zero servings:**

```json
{
  "name": "Test Recipe",
  "source": "manual",
  "servings": 0,
  "instructions": "Stir.",
  "ingredients": []
}
```

Expect: tool-level error citing "servings must be >= 1."

**Error path — unknown parent slug:**

```json
{
  "name": "Test Recipe",
  "source": "manual",
  "parent_recipe_slug": "this-recipe-does-not-exist",
  "servings": 4,
  "instructions": "Stir.",
  "ingredients": []
}
```

Expect: tool-level error mentioning the bad slug AND pointing at `search_recipes`.

---

### `update_recipe`

State-changing, and destructive in a way `create_recipe` is not — the list and block fields replace whole. Use a throwaway dev DB, and seed a recipe with tags, ingredients, and nutrition first (`create_recipe`) so you can see what survives an edit.

Substitute the slug of the recipe you seeded for `test-recipe` throughout.

**Happy path — partial update:**

```json
{
  "slug": "test-recipe",
  "servings": 6,
  "notes": "doubles well"
}
```

Expect: the full updated recipe with `servings: 6` and the new note. Every field you did not send — name, instructions, ingredients, tags, nutrition — comes back exactly as it was.

Note what that means for `servings`: the ingredient amounts are untouched, and `get_shopping_list` divides them by `servings`, so this call quietly cut every per-person quantity by a third. Sending `servings` alone is only correct when the stored count was wrong; a genuine resize has to send a rescaled `ingredients` array in the same call.

**Rename keeps the slug:**

```json
{
  "slug": "test-recipe",
  "name": "Renamed Test Recipe"
}
```

Expect: `name` is the new one and `slug` is still `test-recipe`. Keep using the original slug for `get_recipe` / `create_meal` — a rename never re-derives it.

**Whole-blob replacement — this DELETES the ingredients you omit:**

```json
{
  "slug": "test-recipe",
  "ingredients": [
    {
      "name": "replacement ingredient",
      "amount": { "kind": "single", "value": 2.0 },
      "unit": "cup"
    }
  ]
}
```

Expect: the recipe now has exactly one ingredient. Anything that was there before is gone. Same for `tags`, `instructions`, and `nutrition_per_serving` — a `nutrition_per_serving` carrying only `calories` nulls protein, carbs, fat, and notes.

**Clearing a list:**

```json
{
  "slug": "test-recipe",
  "tags": []
}
```

Expect: `tags` is empty and the ingredients are untouched. `[]` is the only way to clear a list.

**No clear path for scalars:**

```json
{
  "slug": "test-recipe",
  "instructions": "",
  "notes": "   "
}
```

Expect: success, and both fields keep their previous values. An empty or whitespace-only string means "no change", never "blank it".

**Error path — unknown slug:**

```json
{ "slug": "this-recipe-does-not-exist", "servings": 6 }
```

Expect: tool-level error naming the bad slug and pointing at `list_curated_recipes` / `search_recipes` — the same message `create_meal` produces for an unresolvable recipe.

**Error path — empty slug:**

```json
{ "slug": "   ", "servings": 6 }
```

Expect: tool-level error saying `slug` must not be empty or whitespace-only, NOT "no recipe with slug ''".

**Error path — zero servings:**

```json
{ "slug": "test-recipe", "servings": 0 }
```

Expect: tool-level error citing "servings must be >= 1", and the recipe unchanged.

---

### `favorite_recipe`

State-changing, but single-column and reversible — the safest of the write tools to exercise. Seed a recipe first (`create_recipe`) and substitute its slug for `test-recipe` throughout.

**Happy path — favorite it:**

```json
{ "slug": "test-recipe", "is_favorite": true }
```

Expect: the same brief row `search_recipes` returns — slug, name, description, tags, icon, servings, total time, how many times it has been planned, when it was last planned, rating, is_favorite — with `is_favorite: true`. The brief row is deliberately smaller than `get_recipe`'s: it carries enough to confirm the write, not the ingredients or instructions.

Then call `list_curated_recipes`: the recipe now appears at the front of the shortlist. Favorites are listed first and are never truncated.

**Unfavorite it:**

```json
{ "slug": "test-recipe", "is_favorite": false }
```

Expect: `is_favorite: false`, and the recipe drops back out of the favorites tier of `list_curated_recipes`.

**Not a toggle — repeat the same call:**

```json
{ "slug": "test-recipe", "is_favorite": true }
```

Send this twice. Expect: `is_favorite: true` both times. A second call does not flip it back off. This is the difference from the web UI's star button, which toggles — here the value you send is the value you get, so the LLM never has to know the current state first.

**Pairs with the `search_recipes` filter:**

```json
{ "is_favorite": true }
```

Expect: `search_recipes` returns exactly the recipes you favorited above. `is_favorite` counts as a filter on its own, so this is not a rejected bare call.

**Nothing else changes:**

Call `get_recipe` on the slug before and after a `favorite_recipe` call. Expect: every other field — name, servings, ingredients, tags, nutrition, notes, rating — is identical. This tool writes one column.

**Error path — unknown slug:**

```json
{ "slug": "this-recipe-does-not-exist", "is_favorite": true }
```

Expect: tool-level error naming the bad slug and pointing at `list_curated_recipes` / `search_recipes` — the same message `update_recipe` and `create_meal` produce.

**Error path — empty slug:**

```json
{ "slug": "   ", "is_favorite": true }
```

Expect: tool-level error saying `slug` must not be empty or whitespace-only, NOT "no recipe with slug ''".

**Error path — missing `is_favorite`:**

```json
{ "slug": "test-recipe" }
```

Expect: tool-level error naming the missing field. `is_favorite` is required on purpose — a default would be `false`, so an omission would silently unfavorite the recipe instead of failing.

---

### `create_meal`

State-changing. Schedules a meal on a date with per-person serving assignments.

**Happy path — recipe-based serving:**

```json
{
  "date": "2026-05-15",
  "meal_type": "Dinner",
  "servings": [
    {
      "kind": "recipe",
      "person_name": "Alice",
      "recipe_slug": "chicken-pot-pie",
      "servings_count": 1.0
    }
  ]
}
```

Expect: the created meal with the recipe and person resolved to their canonical forms. The MCP boundary normalizes `meal_type` casing — `"dinner"`, `"Dinner"`, and `"DINNER"` all work; the server stores the canonical Title Case form so the planner UI renders correctly (see the cross-boundary conventions note in `CLAUDE.md`).

**Happy path — ad-hoc serving:**

```json
{
  "date": "2026-05-15",
  "meal_type": "Dinner",
  "servings": [
    {
      "kind": "adhoc",
      "person_name": "Alice",
      "items": [
        {
          "name": "leftovers",
          "amount": { "kind": "single", "value": 1.0 },
          "unit": "serving"
        }
      ]
    }
  ]
}
```

Expect: the meal with the ad-hoc ingredient list inline.

**Error path — unknown meal type:**

```json
{
  "date": "2026-05-15",
  "meal_type": "brunch",
  "servings": []
}
```

Expect: tool-level error listing the canonical types (Breakfast, Lunch, Dinner, Snack).

**Error path — unknown person:**

```json
{
  "date": "2026-05-15",
  "meal_type": "Dinner",
  "servings": [
    {
      "kind": "recipe",
      "person_name": "NotARealName",
      "recipe_slug": "any-slug",
      "servings_count": 1.0
    }
  ]
}
```

Expect: tool-level error mentioning the unknown name AND pointing at `list_people`.

## When to upgrade a scenario to an automated test

Add an automated test (don't just keep it here) when:

- The scenario probes a contract — error-message contents, field-omission invariants (e.g. `mcp_token_hash` never on the wire), wire-format expectations.
- You've manually reproduced the same failure twice.
- The scenario covers a regression the team has hit before.

Add at the right layer:

- Service-layer correctness → `server/tests/service_tests.rs`
- Schema validation, normalization, Display contracts → `#[cfg(test)] mod tests` in `server/src/mcp/schemas/recipes.rs` (or sibling files)
- Tool handler plumbing (success/error path mapping, redaction) → `#[cfg(test)] mod tests` in `server/src/mcp/handler.rs`
- End-to-end transport (initialize → notifications/initialized → tools/call) → `server/tests/mcp_auth_plumbing_test.rs`

The Inspector is for the cases that aren't worth automating — first-time UX checks, exploratory poking, "does this _feel_ right."
