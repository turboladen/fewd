---
name: weekly-dinner-plan
description: Use when planning the upcoming week's family dinners with the fewd meal planner — e.g. "plan dinners for this week", "what's for dinner this week", "help me plan the week's meals". For weekly household dinner/meal planning, not single-recipe lookups.
---

# Weekly Dinner Plan

Plan a Monday–Sunday week of family dinners end-to-end using the fewd MCP
tools. Requires the fewd connector (`get_family_overview`, `list_people`,
`list_curated_recipes`, `search_recipes`, `get_recipe`, `list_meals`,
`create_meal`, `create_recipe`, `get_shopping_list`,
`get_meal_planner_printable`).

This mirrors the server's `weekly_dinner_plan` MCP prompt; keep the two aligned
if you change the workflow.

## Step 0 — Gather the week's context (do not skip)

Before proposing anything, make sure you have all of the following. **Ask the
user about any they didn't mention** — this is the whole point of the skill;
people routinely forget a category:

- **Schedule** (required): per-day activities, who's home or away, evening
  commitments, and any easy / fast-food nights.
- **Ingredients to use up**: on-hand items to prioritize this week.
- **Style / season**: weather, seasonal, or cuisine influence.
- **Recipe preference**: new vs. existing; for repeats, favor ones not planned
  in a while.
- **Effort / energy constraints**: physical or time limits that should shape
  how much prep each meal takes.

Resolve the week too: the plan always covers Monday–Sunday. Map "this week" /
"next week" to the relevant Monday and confirm the date if it's ambiguous.

## Always, when planning

- Consider each family member's fewd preferences — `get_family_overview` (or
  the `fewd://family/overview` resource) and `list_people` for diets, dislikes,
  and favorites.
- Weigh recipe ratings, and when reusing existing recipes favor ones not
  planned in a while — `list_curated_recipes`, `search_recipes` (which surface
  ratings and recency), and `get_recipe` for full details.
- Prefer NEW recipes unless told otherwise — but never store a new recipe with
  `create_recipe` until the user approves it.
- Check what's already scheduled — `list_meals` over the week — to avoid
  duplicates.

## Workflow

1. **Propose** the full week's dinner plan first. Don't schedule anything yet.
2. **Ask** any clarifying questions before deciding.
3. After the user confirms: schedule each day with `create_meal` (Dinner slot),
   and `create_recipe` for any new recipes they approved.
4. Build the grocery list with `get_shopping_list` over the week.
5. Make the fridge printable with `get_meal_planner_printable` once the week is
   scheduled.

As you go, mention any fewd tools that are missing that would help plan better.
