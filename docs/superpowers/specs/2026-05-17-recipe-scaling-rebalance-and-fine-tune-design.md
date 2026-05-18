# Recipe Scaling — Ratio Reference and Free-Form Fine-Tune

**Status:** Draft (revised after chef-domain reality check) · **Owner:** Steve Loveless · **Date:** 2026-05-17

## Background

The Scale Recipe form (`src/components/RecipeManager.tsx`, backed by `server/src/services/recipe_scaler.rs`) currently lets a user pick a target serving count, previews the rescaled ingredient list, and offers a manual-override input on any ingredient whose discrete-unit count came out fractional (e.g. `3.75 eggs`). The save buttons commit either as a new recipe or in place.

Two limitations surface in real use:

1. **Manual overrides don't propagate.** Editing `4.5 eggs → 5` updates only that row. Other ingredients keep their previously-scaled values, and the user has no visibility into how far that puts the recipe from its original ratios.
2. **Only flagged rows are editable.** A user who wants to round `1.67 cups milk → 2 cups` for kitchen practicality has no input affordance for that row.

## Design philosophy: match how kitchens actually scale

An earlier draft of this spec proposed a "Rebalance + clean-multiplier" mechanism that mathematically guaranteed proportional rescaling never produced a fractional discrete unit. That design was technically elegant but didn't match how real kitchens scale recipes:

- Professional cookbooks use baker's percentages and weight-based formulation (eggs as grams, not count) — the discrete-fractional problem doesn't exist for them at all.
- Home and pro cooks who don't use weights just **round to whole eggs and accept small ratio drift**. A pancake recipe that's 7–10% richer on eggs than the original is still indistinguishable in the pan.
- Scaling beyond modest factors stops being pure math anyway (spice non-linearity, leavener behavior, browning surface area, salt to taste).

So the simpler design below makes minor ratio drift the normal case, not a flagged exception. The user gets visibility (a Ratio column) and full editing freedom, but nothing hunts for "mathematically optimal" multipliers because real cooking doesn't optimize for that.

Weight-based scaling (the chef's actual answer) is captured separately as `fewd-bhd` — a potential future pivot if the simpler design proves insufficient.

## Design

### Entry point unchanged

The existing "scale to X servings" flow remains the big-picture entry. User types a target, hits Preview, gets the current backend response from `recipe_scaler::scale_ingredients`. The composable `fewd-2bp` work auto-rounds fractional discrete units inside that response. Everything below operates on the preview state after those upstream steps.

### Three-column ingredient display

Each ingredient row becomes a three-column grid:

```
Original          Current (editable)     Ratio
3       eggs      [ 5 ]                  1.667×
1 cup   milk      [ 1.67 ] cups          1.667×
0.25 tsp salt     [ 0.42 ] tsp           1.667×
2 tbsp  sugar     [ 3.33 ] tbsp          1.667×
```

- **Original** is read-only, sourced from the recipe at its native serving count.
- **Current** is editable on every row whose amount is a `single`. Rows whose amount is a `range` (e.g. `1–2 cups`) render as static text — committing a range-typed amount through a single input would silently drop the max bound, so range rows are held out of the editable surface rather than collapsed on edit.
- **Ratio** is computed as `current / original`. When all rows share the same ratio, the recipe is in a strictly-proportional state. When they diverge, the user sees exactly which rows have drifted and by how much.

Rows whose backend response marked them as fractional-discrete carry a subtle visual cue (a soft amber border on the input) so the user can find them at a glance. This is a discoverability hint, not a "must-fix" gate — the user can ignore it and save the recipe as-is. It's much lighter than the previous design's full-row amber background + "fractional" pill, which framed flagged values as broken; the new treatment frames them as worth noticing.

The Ratio column is the central feature. It serves two purposes:

1. **Reference during fine-tune** — as the user nudges values, they see real-time per-ingredient deviation from the proportional original.
2. **Sanity check** — divergent ratios surface where intentional drift exists ("ah, I rounded milk up but didn't touch the others"), making the recipe being saved transparent.

The column has no gating power: it doesn't disable buttons, doesn't flag the recipe as a "variant," doesn't force the user toward any particular value. It just shows.

### Free-form editing — no Rebalance machinery

Every row's Current value is editable at any time. Edits are purely local: they update only the row being edited. No automatic cascading, no "rebalance everything" button, no clean-multiplier search.

This is the same gesture pattern home cooks already use mentally: scale to the target, then nudge individual values for kitchen practicality (round milk up, round salt down, round eggs to whole numbers). The system gets out of the way and shows the resulting ratios so the user can spot anything they didn't intend.

If the user wants to apply a different overall ratio, they re-enter the target servings and hit Preview again. Hitting Preview discards any pending local edits and replaces all rows with freshly-scaled values from the backend — by design, since the user is asking for a fresh proportional rescale. The existing Preview button + serving-target input is the proportional-rescale lever; the new editable rows are the kitchen-adjustment lever. Two distinct tools, no overlap.

### Save labels — keep them simple

Today's labels (`Save as New Recipe`, `Update This Recipe`) remain. They accurately describe what's happening: save the recipe as currently displayed, at the entered target servings. Ratio drift between rows is part of normal recipe scaling and doesn't warrant special labeling.

If the user wants to verify what they're saving, the Ratio column is right there.

## Implementation notes

### Backend

No backend changes for this work. The existing `recipe_scaler::scale_ingredients` continues to be the scaling source of truth. `fewd-2bp` is the composable upstream change that improves what comes back from that function.

### Frontend

`RecipeManager.tsx` Scale Recipe panel changes:

- Replace the current ingredient list (lines ~669–716, the section that conditionally renders editable input vs. static span based on `flaggedIndices.has(i)`) with a three-column grid using Tailwind grid utilities.
- Make `Current` an editable `<NumberInput>` (the existing string-buffered component) for `single`-typed amounts; render `range`-typed amounts as static text so the max bound can't be silently dropped. Wire onChange through `handleIngredientChange`. No negatives (`min={0}`), allow decimals (`step='any'`).
- Add `Ratio` column rendering `current / original` formatted as e.g. `1.67×` (2 decimal places, trailing zeros trimmed).
- Keep `flaggedIndices` derived from `preview.flagged` and apply a soft `border-amber-300` to the input on flagged rows — discoverable signal, no "must-fix" weight.
- Drop the previous heavy flagged-row treatment (full-row amber background, "fractional" pill). The lighter `border-amber-300` input treatment above replaces it. `fewd-0mt` (the original bug about the highlight stretching full-width) is moot under the new treatment since the styling lives on the input rather than the row, and can be closed when this lands.
- The warning banner ("Some ingredients have fractional amounts for discrete units…") is only meaningful while initial Preview can return fractional discrete values. If this work ships **after** `fewd-2bp` (auto-round), remove the banner — there's nothing to warn about. If this work ships **before** `fewd-2bp`, leave the banner in place; remove it when `fewd-2bp` lands.

### Test plan

Frontend (Vitest + RTL):

- Every ingredient row renders an editable Current input.
- Editing a row updates only that row's Current value.
- Ratio column shows `current / original` for each row, formatted as `Nx`.
- All rows showing the same ratio is the proportional state; divergent ratios render as-is with no special UI treatment.
- Saving as new / updating in place uses the current displayed values.

Manual smoke:

- Scale Swedish pancakes 4 → 5 servings. If `fewd-2bp` has shipped, eggs land at 4 (auto-rounded) with Ratio ≈ 1.33×; other rows at 1.25×. If `fewd-2bp` hasn't shipped yet, eggs land at 3.75 with Ratio = 1.25× across all rows. In either case: edit milk from `1.25` cups → `1.5` cups; Ratio column for milk updates to 1.5× while other rows stay unchanged. Save as new and verify the saved recipe matches the displayed state.

## Out of scope (other beads)

- **Auto-rounding during initial scale** (`fewd-2bp`): when initial Preview produces fractional discrete, round at the service layer. Composable with this design and arguably should ship first.
- **Highlight styling** (`fewd-0mt`): this design removes the flagged-highlight UI entirely, making the bug moot. Close `fewd-0mt` when this lands.
- **Weight-based scaling for discrete ingredients** (`fewd-bhd`): the chef-domain answer to fractional eggs (express in grams, scale continuously). Logged as a potential future pivot.
- **Shopping-list rounding** (`fewd-p3j`): different service, different problem.

## Relationships

- **Refines:** existing Scale Recipe form
- **Closes:** `fewd-0mt` (flagged-row highlight bug, made moot)
- **Related to:** `fewd-2bp` (auto-round at Preview — composable, should ship first), `fewd-bhd` (weight pivot — future), `fewd-p3j` (shopping-list rounding, separate service)
- **Service-layer locality:** frontend-only change in `RecipeManager.tsx`. No backend, no DB migration, no new HTTP route, no schema change.
