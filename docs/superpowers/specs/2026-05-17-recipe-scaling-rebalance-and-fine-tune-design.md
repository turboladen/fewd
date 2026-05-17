# Recipe Scaling — Rebalance and Fine-Tune

**Status:** Draft · **Owner:** Steve Loveless · **Date:** 2026-05-17

## Background

The Scale Recipe form (`src/components/RecipeManager.tsx`, backed by `server/src/services/recipe_scaler.rs`) currently lets a user pick a target serving count, previews the rescaled ingredient list, and offers a manual-override input on any ingredient whose discrete-unit count came out fractional (e.g. `3.75 eggs`). The save buttons then commit either as a new recipe or in place.

Two limitations surface in real use:

1. **Manual overrides don't propagate.** Editing `4.5 eggs → 5` updates only that row. The rest of the recipe still reflects the original serving-target ratio, so the recipe being saved is internally inconsistent.
2. **No reference for "how far from the original am I?"** Once a user starts tweaking values, there's no way to see per-ingredient deviation from the proportional baseline. The user has to mentally track ratios across rows.

These limitations make the save buttons feel dishonest — committing a recipe that's "in between" two ratios isn't really saving the scaled recipe the user thinks they're saving.

## The mathematical constraint that drives the design

Proportional rescaling applies a single multiplier `k` to every ingredient. For discrete-unit ingredients (eggs, onions, cloves — anything `is_discrete_unit` in `recipe_scaler.rs` returns true for), the result is a real recipe only when `k × original_count` is a whole number for every such ingredient.

For a recipe with discrete ingredients `[3 eggs, 1 onion]`, the only clean multipliers are integers (1×, 2×, 3×, …) — because `k × 1 = k` must be integer. For `[6 eggs, 4 cloves, 2 onions]`, the GCD is 2, so multiples of ½ also work. In the general case, valid `k` values form a regular grid determined by `1 / GCD(discrete_counts)`.

Most home recipes contain at least one "1-unit" discrete ingredient (1 onion, 1 lemon, 1 head garlic), which means **clean multipliers collapse to integers only**. This is why home cooks naturally think in halves, doubles, and triples — the math forces it.

This constraint is the design's organizing principle: **Rebalance must never produce a fractional discrete unit.** If the user's edit implies a non-clean multiplier, Rebalance refuses to fire and offers the nearest clean snap instead. If the user wants a serving count that no clean multiplier reaches, they can fine-tune individual ingredients — but they cross a clearly-marked boundary out of strict proportional scaling.

## Design

### Entry point unchanged

The existing "scale to X servings" flow remains the big-picture entry. User types a target, hits Preview, gets the current backend response from `recipe_scaler::scale_ingredients`. Everything below operates on that preview state.

### Three-column ingredient display

Each ingredient row gains a Ratio column:

```
Original          Current (editable)     Ratio
3       eggs      [ 5 ]                  1.667×
1 cup   milk      [ 1.67 ] cups          1.667×
0.25 tsp salt     [ 0.42 ] tsp           1.667×
```

- **Original** is read-only, sourced from the recipe at its native serving count.
- **Current** is editable on every row (not just flagged ones). Volume/weight units, discrete units, all editable.
- **Ratio** is computed as `current / original`. When all rows share the same ratio, the recipe is in a strictly-proportional state. Divergent ratios make per-ingredient deviation visible at a glance.

The Ratio column also serves as a debugger during fine-tune — a user who sees `1.5× / 1.667× / 1.667×` immediately spots the under-scaled outlier.

### Rebalance button — gated by the clean-multiplier constraint

Rebalance is triggered by edits to **discrete-unit ingredients** only (eggs, cloves, onions — anything `is_discrete_unit` returns true for). Those edits express the user's "I want N whole units" intent and define a candidate anchor. Edits to volume/weight ingredients (cups, tbsp, grams) are treated as fine-tunes and do NOT surface a Rebalance affordance — by definition those units can already hold fractional values without breaking the recipe.

When the user edits a discrete-unit ingredient, the system computes the implied multiplier `k = new_value / original_value` and checks whether `k` is clean for this recipe (i.e. `k × original_count ∈ ℤ` for every discrete-unit ingredient, using a floating-point tolerance of `1e-6`).

**Clean case:** Button enabled. Text reflects the anchor: `"Rebalance to 5 eggs (≈6.67 servings)"`. Click rescales every row at multiplier `k`, target-servings field updates.

**Non-clean case:** Button disabled, replaced with an explanation panel:

```
Can't proportionally rebalance to 4 eggs —
onions would land at 2.67.

Nearest clean: 2× (6 eggs, 2 onions)   [Apply]
Show other clean options ↓
```

The "Apply" snaps to the nearest clean multiplier. "Show other clean options" expands the next several clean multipliers (default: the 5 nearest to the implied `k`, capped at `0.25× ≤ k ≤ 4×` to keep results household-sensible). If the implied `k` is so far outside this window that no clean multipliers fall inside it, the panel offers only the fine-tune path.

This guarantees the post-rebalance state is always a real recipe with intact proportions.

### Fine-tune — explicit step outside proportional scaling

If the user wants a result that no clean multiplier provides, they edit rows freely. Edits to non-discrete (volume/weight) rows are always fine-tunes — no Rebalance affordance appears for them. Edits to discrete-unit rows continue to surface Rebalance for as long as the user wants to keep retrying proportional rescaling; the user crosses into pure-fine-tune territory simply by ignoring the Rebalance affordance and continuing to edit. The Ratio column makes the deviation legible.

Fine-tune and Rebalance compose: a user can scale to a target → Rebalance to a clean multiplier → fine-tune individual values from there. The final state is whatever rows the user has set.

### Save labels reflect the actual state being saved

Today's labels (`Save as New Recipe`, `Update This Recipe`) read as if the form is always saving a clean rescale, which is misleading once the user has fine-tuned.

New labels derive from the current state:

- All ratios equal (within `1e-6` tolerance) AND target servings is the current input value:
  `"Save as new {N}-serving recipe"`, `"Update recipe to {N} servings"`
- Post-Rebalance (target servings updated to implied value):
  `"Save as new {N}-serving recipe (rebalanced to {anchor})"`
- Post-fine-tune (ratios diverge beyond `1e-6` tolerance):
  `"Save as new {N}-serving recipe — variant of original"`, `"Update recipe — variant of original"`

The text doesn't try to be clever; it just describes what's being committed. The user picks the operation that matches their intent.

## Implementation notes

### Backend

A small new helper in `recipe_scaler.rs` (or a sibling `clean_multipliers.rs`):

```rust
/// Returns clean multipliers for this recipe near the given target.
/// `target` is the multiplier the user implied (e.g. 1.667 for 5 eggs from 3).
/// Returns up to `limit` clean multipliers sorted by distance to target.
pub fn nearby_clean_multipliers(
    ingredients: &[IngredientDto],
    target: f64,
    limit: usize,
) -> Vec<f64> { /* ... */ }

/// True iff `k` produces a whole-number count for every discrete-unit
/// ingredient in `ingredients`.
pub fn is_clean_multiplier(ingredients: &[IngredientDto], k: f64) -> bool { /* ... */ }
```

The existing `scale_ingredients` continues to be the only mutation entry point — Rebalance is just "call scale_ingredients again with a new ratio." No new endpoint required; the frontend reuses `previewMutation`.

### Frontend

`RecipeManager.tsx` Scale Recipe panel changes:

- Replace the single-column ingredient list (lines ~669–716) with a three-column grid (Original / Current / Ratio).
- Replace the conditional `flaggedIndices.has(i)` editable/static branching with a uniform editable Current column.
- Add a Rebalance button + clean-snap panel in the form's action area (above the existing Save row).
- Update save-button labels per the state-derived rules above.

The `editedIngredients` state already captures local edits; the Ratio column reads off `editedIngredients[i].amount` vs. `parsed.ingredients[i].amount`. The Rebalance button reuses `previewMutation` with the derived target-servings value.

### Test plan

Backend (`recipe_scaler.rs` unit tests):

- `nearby_clean_multipliers` returns the right set for common recipes:
  - `[3 eggs, 1 onion]` near 1.5× → `[1, 2]` (or further if `limit` allows).
  - `[6 eggs, 4 cloves]` near 1.3× → `[1, 1.5]` (half-multiples valid here).
  - Recipe with no discrete units → every `k` is clean; return target as the only result.
- `is_clean_multiplier`: confirms whole-number landing for every discrete count.

Frontend (Vitest + RTL):

- Editing a row updates the Ratio column.
- Ratio column shows divergent ratios when fine-tuning.
- Rebalance button enables/disables correctly based on clean-multiplier check.
- Clicking Rebalance updates all rows and the target-servings field.
- Snap-to-clean Apply button updates state to the snapped multiplier.
- Save button labels switch between proportional / rebalanced / variant text per state.

End-to-end (manual smoke):

- Scale Swedish pancakes (1 discrete: eggs) 4 → 5 servings, bump eggs 3.75 → 5, Rebalance, save as new. Verify saved recipe = `5 eggs, 1.67 cups milk, …` with target servings ≈7.
- Scale recipe with `[3 eggs, 1 onion]` 4 → 5 servings, bump eggs to 4. Verify Rebalance is disabled, snap offers 2× (8 servings). Apply snap and save.

## Out of scope (future beads)

- **Auto-rounding during initial scale** (`fewd-2bp`): when the initial Preview lands at fractional discrete, auto-round to the nearest clean multiplier rather than leaving the user to manually edit. Composable with this design — Rebalance still does what's described here for explicit overrides.
- **Highlight styling** (`fewd-0mt`): the amber fractional-row callout's full-width stretch is a separate visual bug. This design's three-column layout will change the surrounding structure enough that `fewd-0mt` should be re-evaluated against the new layout before being addressed.
- **Non-flagged value rounding to grocery realism** (`fewd-p3j`): the shopping-list aggregator's separate rounding rules — different problem, different service.

## Relationships

- **Refines:** existing Scale Recipe form
- **Related to:** `fewd-2bp` (recipe scaler auto-round), `fewd-0mt` (highlight styling), `fewd-p3j` (shopping-list rounding)
- **Service-layer locality:** all changes confined to `recipe_scaler.rs` (helpers only) and `RecipeManager.tsx` (UI). No DB migration, no new HTTP route, no schema change.
