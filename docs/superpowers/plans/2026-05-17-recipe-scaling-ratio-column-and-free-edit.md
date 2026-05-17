# Recipe Scaling — Ratio Column + Free-Form Editing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Scale Recipe panel's row-rendering block with a 3-column grid (Original | Current/editable | Ratio) where every row is editable and the ratio column shows per-ingredient deviation from the original.

**Architecture:** Frontend-only change inside `src/components/RecipeManager.tsx` (the `ScaleRecipePanel` function). Two small pure helpers (`ingredientRatio`, `formatRatio`) live in `src/types/recipe.ts` next to the existing `formatAmount`. The flagged-highlight machinery gets removed; the warning banner stays (per spec, until `fewd-2bp` lands). No backend, no schema, no migration.

**Tech Stack:** React 18 + TypeScript + Vitest + React Testing Library + Tailwind. The Vitest test setup uses `installFetchMock` / `installStreamMock` from `src/test/fetchMock.ts` and `src/test/streamMock.ts`. The recipe factory is at `src/test/factories.ts` (function `makeRecipe`).

**Spec:** `docs/superpowers/specs/2026-05-17-recipe-scaling-rebalance-and-fine-tune-design.md`

**Bead:** fewd-b3x

---

## Setup

- [ ] **Step 0: Confirm starting state on `main`, branch off**

```bash
git status                                  # expect clean, on main
git log --oneline -3                        # expect dd11950 + 8171b47 (spec commits) at HEAD
git checkout -b fewd-b3x/ratio-column-free-edit
```

Expected: branch `fewd-b3x/ratio-column-free-edit` created from main.

---

## File Structure

Files this plan touches:

| File                                    | Role                                                                                                                                                                                                   |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src/types/recipe.ts`                   | Add `ingredientRatio(current, original)` and `formatRatio(ratio)` pure helpers next to `formatAmount`.                                                                                                 |
| `src/types/recipe.test.ts`              | Unit tests for the two new helpers.                                                                                                                                                                    |
| `src/components/RecipeManager.tsx`      | Rewrite the `ScaleRecipePanel`'s row-rendering block (~lines 660–716): 3-column grid, every row editable, ratio column, remove flagged-highlight machinery and the `flaggedIndices` Set. Banner stays. |
| `src/components/RecipeManager.test.tsx` | New `describe('ScaleRecipePanel')` block with rendering + interaction tests.                                                                                                                           |

No files created; only modifications.

---

## Task 1: Add `ingredientRatio` and `formatRatio` helpers

**Files:**

- Modify: `src/types/recipe.ts` (add two functions next to `formatAmount` around line 219)
- Modify: `src/types/recipe.test.ts` (add new `describe` blocks)

### Background

`ingredientRatio(current, original)` returns a positive number (or `null` if either side is non-numeric, e.g. division by zero). For `single` amounts, use `current.value / original.value`. For `range` amounts on either side, use the `.min` value as the reference (post-edit values always collapse to single, so a `range` will only appear when the row is unedited — `.min` matches what the user sees in the input field).

`formatRatio(ratio: number | null)` returns a display string. Examples:

- `1.0` → `"1×"`
- `1.667` → `"1.67×"`
- `1.5` → `"1.5×"`
- `0.5` → `"0.5×"`
- `null` → `"—"` (em dash for "ratio undefined")

Note the multiplication sign is `×` (U+00D7, Unicode multiplication sign), not the letter `x`.

### Steps

- [ ] **Step 1: Write failing tests for `ingredientRatio`**

Append to `src/types/recipe.test.ts` (after the last existing test):

```typescript
import { formatAmount, formatRatio, ingredientRatio } from './recipe'
import type { IngredientAmount } from './recipe'

describe('ingredientRatio', () => {
  it('returns current.value / original.value for single amounts', () => {
    const original: IngredientAmount = { type: 'single', value: 3 }
    const current: IngredientAmount = { type: 'single', value: 5 }
    expect(ingredientRatio(current, original)).toBeCloseTo(1.667, 3)
  })

  it('uses min on either side when the amount is a range', () => {
    const original: IngredientAmount = { type: 'range', min: 2, max: 4 }
    const current: IngredientAmount = { type: 'single', value: 3 }
    expect(ingredientRatio(current, original)).toBe(1.5)
  })

  it('returns null when the original value is zero (would divide by zero)', () => {
    const original: IngredientAmount = { type: 'single', value: 0 }
    const current: IngredientAmount = { type: 'single', value: 1 }
    expect(ingredientRatio(current, original)).toBeNull()
  })

  it('returns 1 when current matches original exactly', () => {
    const original: IngredientAmount = { type: 'single', value: 2 }
    const current: IngredientAmount = { type: 'single', value: 2 }
    expect(ingredientRatio(current, original)).toBe(1)
  })
})

describe('formatRatio', () => {
  it('renders 1.0 as "1×"', () => {
    expect(formatRatio(1.0)).toBe('1×')
  })

  it('renders 1.667 as "1.67×" (two decimals, no trailing zero trim needed)', () => {
    expect(formatRatio(1.667)).toBe('1.67×')
  })

  it('trims trailing zeros after the decimal', () => {
    expect(formatRatio(1.5)).toBe('1.5×')
    expect(formatRatio(1.5000001)).toBe('1.5×')
  })

  it('handles sub-1 ratios', () => {
    expect(formatRatio(0.5)).toBe('0.5×')
  })

  it('renders null as em dash', () => {
    expect(formatRatio(null)).toBe('—')
  })
})
```

- [ ] **Step 2: Run tests, confirm they fail**

```bash
bun run test src/types/recipe.test.ts -- --run
```

Expected: failures with `ingredientRatio is not exported from './recipe'` / `formatRatio is not exported from './recipe'`.

- [ ] **Step 3: Implement `ingredientRatio` and `formatRatio` in `src/types/recipe.ts`**

Add these two functions immediately after `formatAmount` (around line 220):

```typescript
/**
 * Returns the ratio of `current` to `original` as a positive number, or
 * `null` if the original side has a zero reference value (avoids div-by-zero
 * UI noise). For `range` amounts on either side, uses `.min` as the
 * reference value — post-edit values always collapse to `single`, so a
 * `range` only appears on an untouched preview row where `.min` matches
 * what the user sees rendered in the input.
 */
export function ingredientRatio(
  current: IngredientAmount,
  original: IngredientAmount,
): number | null {
  const originalRef = original.type === 'single' ? original.value : original.min
  if (originalRef === 0) return null
  const currentRef = current.type === 'single' ? current.value : current.min
  return currentRef / originalRef
}

/**
 * Display string for a ratio value. Uses the Unicode multiplication sign
 * (×, U+00D7), formats to up to two decimal places, trims trailing zeros,
 * and renders `null` as an em dash for ratios that can't be computed.
 */
export function formatRatio(ratio: number | null): string {
  if (ratio === null) return '—'
  const rounded = Math.round(ratio * 100) / 100
  const formatted = rounded % 1 === 0
    ? String(rounded)
    : rounded.toFixed(2).replace(/0+$/, '').replace(/\.$/, '')
  return `${formatted}×`
}
```

- [ ] **Step 4: Run tests, confirm they pass**

```bash
bun run test src/types/recipe.test.ts -- --run
```

Expected: all tests in the two new `describe` blocks pass.

- [ ] **Step 5: Type-check (Vitest doesn't run tsc, per CLAUDE.md)**

```bash
bunx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add src/types/recipe.ts src/types/recipe.test.ts
git commit -m "$(cat <<'EOF'
feat(fewd-b3x): add ingredientRatio + formatRatio helpers

Pure helpers used by the upcoming Scale Recipe panel rewrite to display
per-ingredient deviation from the original recipe's amounts. ingredientRatio
returns null on a zero reference to avoid div-by-zero UI noise; formatRatio
uses the Unicode multiplication sign and trims trailing zeros.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Rewrite `ScaleRecipePanel` row rendering as a 3-column grid

**Files:**

- Modify: `src/components/RecipeManager.tsx` (replace lines ~660–716; remove `flaggedIndices` derivation at line 618)
- Modify: `src/components/RecipeManager.test.tsx` (add `describe('ScaleRecipePanel')` block at the end)

### Background

Today's row-rendering branches on `flaggedIndices.has(i)` to choose between an editable input and a static span. The new design renders every row with an editable input, plus a Ratio column.

**About the warning banner:** the spec says to keep the "Some ingredients have fractional amounts for discrete units…" banner if this work ships **before** `fewd-2bp` (which it will, since `fewd-2bp` is still open). The banner remains accurate — its body text ("You can adjust them below") is true in either UI variant. The plan keeps the banner conditional on `preview.flagged.length > 0`, exactly as today. It can be removed later in a one-line follow-up when `fewd-2bp` lands.

The `ScalingPreviewAltRow` sub-component (for chained `or_alternative` rendering) is unaffected; it still renders below each row when present.

`flaggedIndices` is no longer used by the panel (the conditional row-styling and input-vs-span branching are gone). We drop the `flaggedIndices` Set but keep `preview` itself (it holds `ingredients`) and keep `preview.flagged` (still consumed by the banner conditional).

### Column widths

Use Tailwind grid utilities. Target a layout that hugs content reasonably and stays consistent regardless of viewport width:

```
grid-cols-[6rem_minmax(6rem,8rem)_3rem_1fr_4rem]
        ^original  ^input        ^unit  ^label  ^ratio
```

This keeps the row width content-sized (not stretched full-panel) and avoids `fewd-0mt`-style stretching.

### Steps

- [ ] **Step 1: Write failing test — every row renders an editable input**

Append to `src/components/RecipeManager.test.tsx`:

```typescript
// Add to existing imports at the top of the file
import type { ParsedRecipe, ScaleResult } from '../types/recipe'
import { ScaleRecipePanel } from './RecipeManager'

describe('ScaleRecipePanel', () => {
  function makeParsed(): ParsedRecipe {
    const recipe = makeRecipe({
      id: 'r1',
      name: 'Test Recipe',
      servings: 4,
      ingredients: [
        { name: 'eggs', amount: { type: 'single', value: 3 }, unit: '' },
        { name: 'milk', amount: { type: 'single', value: 1 }, unit: 'cup' },
      ],
    })
    // parseRecipe is already imported in the existing test file's chain;
    // if not, import it from '../types/recipe'.
    return {
      ...recipe,
      prep_time: null,
      cook_time: null,
      total_time: null,
      portion_size: null,
      nutrition_per_serving: null,
      tags: [],
      ingredients: recipe.ingredients,
    } as ParsedRecipe
  }

  function makeScaleResult(): ScaleResult {
    return {
      ingredients: [
        { name: 'eggs', amount: { type: 'single', value: 3.75 }, unit: '' },
        { name: 'milk', amount: { type: 'single', value: 1.25 }, unit: 'cup' },
      ],
      flagged: [
        { index: 0, name: 'eggs', scaled_value: 3.75, unit: '' },
      ],
    }
  }

  it('renders an editable input for every ingredient row after Preview', async () => {
    const parsed = makeParsed()
    // The component triggers Preview via the mutation hook; we mock the
    // backend response so editedIngredients populates.
    mockJson('POST', '/api/recipes/r1/scale', makeScaleResult())

    renderWithProviders(
      <ScaleRecipePanel
        parsed={parsed}
        onSaveAsNew={() => {}}
        onUpdateInPlace={() => {}}
        onCancel={() => {}}
      />,
    )

    // Bump target servings 4 -> 5 and click Preview
    const servingsInput = screen.getByDisplayValue('4')
    fireEvent.change(servingsInput, { target: { value: '5' } })
    fireEvent.click(screen.getByRole('button', { name: /preview/i }))

    // Both ingredient rows should have editable inputs after preview lands
    await waitFor(() => {
      expect(screen.getByDisplayValue('3.75')).toBeInTheDocument()
      expect(screen.getByDisplayValue('1.25')).toBeInTheDocument()
    })
  })
})
```

- [ ] **Step 2: Run the new test, confirm it fails**

```bash
bun run test src/components/RecipeManager.test.tsx -- --run -t "renders an editable input for every ingredient row"
```

Expected: failure. Today only the flagged (eggs) input renders; the milk row renders a static `<span>`, so `getByDisplayValue('1.25')` will throw.

- [ ] **Step 3: Replace the row-rendering block + remove `flaggedIndices`**

Edit `src/components/RecipeManager.tsx`.

**3a. Remove the `flaggedIndices` derivation.**

Delete the line that reads:

```typescript
const flaggedIndices = new Set(preview?.flagged.map((f) => f.index) ?? [])
```

(Currently line 618. The exact line number may have drifted; locate by the `flaggedIndices` identifier.)

**3b. Replace the row-rendering section (the `<div className='space-y-1 mb-4'>` block and its children).**

KEEP the surrounding pieces:

- The `{preview && editedIngredients && (` opener and its `<>` fragment.
- The `{preview.flagged.length > 0 && (` warning banner — unchanged. Spec keeps the banner until `fewd-2bp` lands.
- The `{error && (` block, the action-buttons row, and the closing tags.

REPLACE only the ingredient-list `<div className='space-y-1 mb-4'>...</div>` block.

Current ingredient-list block to delete (everything from the opening `<div className='space-y-1 mb-4'>` through its closing `</div>`):

```jsx
<div className='space-y-1 mb-4'>
  {editedIngredients.map((ing, i) => (
    <div key={i}>
      <div
        className={`flex gap-2 items-center text-sm ${
          flaggedIndices.has(i)
            ? 'bg-amber-50 border border-amber-200 rounded p-1'
            : 'p-1'
        }`}
      >
        {flaggedIndices.has(i)
          ? (
            <input
              type='number'
              step='any'
              value={ing.amount.type === 'single'
                ? ing.amount.value
                : (ing.amount as { type: 'range'; min: number; max: number }).min}
              onChange={(e) => {
                const val = parseFloat(e.target.value) || 0
                handleIngredientChange(i, {
                  ...ing,
                  amount: { type: 'single', value: val },
                })
              }}
              className='input-sm w-16 border-amber-300'
            />
          )
          : (
            <span className='font-medium w-16 text-right'>
              {formatAmount(ing.amount)}
            </span>
          )}
        <span className='text-stone-500 w-12'>{ing.unit}</span>
        <span>{formatIngredientLabel(ing)}</span>
        {flaggedIndices.has(i) && (
          <span className='text-amber-600 text-xs ml-auto'>fractional</span>
        )}
      </div>
      {
        /* Alternative shown as a static sub-row — not editable here
          because the fractional-rounding workflow only applies to
          primary ingredients. */
      }
      {ing.or_alternative && <ScalingPreviewAltRow ingredient={ing.or_alternative} />}
    </div>
  ))}
</div>
```

Replace with:

```jsx
<div className='space-y-1 mb-4'>
  <div
    className='grid grid-cols-[6rem_minmax(6rem,8rem)_3rem_1fr_4rem] gap-2 items-center text-xs text-stone-500 px-1'
    aria-hidden
  >
    <span className='text-right'>Original</span>
    <span>Current</span>
    <span />
    <span />
    <span className='text-right'>Ratio</span>
  </div>
  {editedIngredients.map((ing, i) => {
    const original = parsed.ingredients[i]
    const ratio = original
      ? ingredientRatio(ing.amount, original.amount)
      : null
    const inputValue = ing.amount.type === 'single'
      ? ing.amount.value
      : ing.amount.min
    return (
      <div key={i}>
        <div className='grid grid-cols-[6rem_minmax(6rem,8rem)_3rem_1fr_4rem] gap-2 items-center text-sm p-1'>
          <span className='font-medium text-right text-stone-600'>
            {original ? formatAmount(original.amount) : '—'}
          </span>
          <input
            type='number'
            step='any'
            value={inputValue}
            onChange={(e) => {
              const val = parseFloat(e.target.value) || 0
              handleIngredientChange(i, {
                ...ing,
                amount: { type: 'single', value: val },
              })
            }}
            className='input-sm'
            aria-label={`${ing.name} amount`}
          />
          <span className='text-stone-500'>{ing.unit}</span>
          <span>{formatIngredientLabel(ing)}</span>
          <span className='text-right text-stone-500 tabular-nums'>
            {formatRatio(ratio)}
          </span>
        </div>
        {ing.or_alternative && (
          <ScalingPreviewAltRow ingredient={ing.or_alternative} />
        )}
      </div>
    )
  })}
</div>
```

**3c. Add `ingredientRatio` and `formatRatio` to the existing imports from `../types/recipe`.**

Find the existing import block (lines ~22–28):

```typescript
import {
  formatAmount,
  formatIngredientLabel,
  formatServings,
  formatTime,
  parseRecipe,
} from '../types/recipe'
```

Replace with:

```typescript
import {
  formatAmount,
  formatIngredientLabel,
  formatRatio,
  formatServings,
  formatTime,
  ingredientRatio,
  parseRecipe,
} from '../types/recipe'
```

- [ ] **Step 4: Run the new test, confirm it passes**

```bash
bun run test src/components/RecipeManager.test.tsx -- --run -t "renders an editable input for every ingredient row"
```

Expected: pass.

- [ ] **Step 5: Run the full existing suite to confirm no regressions**

```bash
bun run test -- --run
```

Expected: all tests pass. The existing `ScalingPreviewAltRow` test continues to work (untouched).

- [ ] **Step 6: Type-check**

```bash
bunx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add src/components/RecipeManager.tsx src/components/RecipeManager.test.tsx
git commit -m "$(cat <<'EOF'
feat(fewd-b3x): rewrite ScaleRecipePanel rows as 3-column editable grid

Every ingredient row now renders Original | Current (editable) | Ratio.
Removes the flagged-only editable / static-span branching, the amber
flagged-row highlight, the 'fractional' pill, and the flaggedIndices
derivation. Ratio column uses the new formatRatio helper to surface
per-ingredient deviation from the original recipe.

The 'Some ingredients have fractional amounts' warning banner stays
(per the spec, until fewd-2bp lands) — it remains accurate, since
preview can still return fractional discrete values until the service
layer auto-rounds.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Add behavior tests for ratio updates and Re-Preview reset

**Files:**

- Modify: `src/components/RecipeManager.test.tsx` (append two more `it` blocks inside the `describe('ScaleRecipePanel')`)

### Background

Two behaviors worth pinning explicitly:

1. **Ratio updates as the user edits** — when the milk input changes from `1.25` → `1.5`, the milk row's ratio cell flips from `1.25×` → `1.5×`, while the eggs row's ratio cell stays at its scaled value (`1.25×` because preview returned 3.75 / 3 original).
2. **Re-Preview discards local edits** — after a user edits a row, clicking Preview again with a new target wipes those edits and replaces all rows with freshly-scaled values from the new backend response. The existing `handlePreview` already does `setEditedIngredients(null)` before mutating, so this is verification, not a fix — but the spec calls it out and pinning a test prevents future regression.

### Steps

- [ ] **Step 1: Write the two failing tests**

Append inside the existing `describe('ScaleRecipePanel')` block (after the test from Task 2):

```typescript
it('updates only the edited row’s ratio cell when the user changes its value', async () => {
  const parsed = makeParsed()
  mockJson('POST', '/api/recipes/r1/scale', makeScaleResult())

  renderWithProviders(
    <ScaleRecipePanel
      parsed={parsed}
      onSaveAsNew={() => {}}
      onUpdateInPlace={() => {}}
      onCancel={() => {}}
    />,
  )

  fireEvent.change(screen.getByDisplayValue('4'), { target: { value: '5' } })
  fireEvent.click(screen.getByRole('button', { name: /preview/i }))

  await waitFor(() =>
    expect(screen.getByDisplayValue('1.25')).toBeInTheDocument()
  )

  // Initially both rows show 1.25× (3.75/3 and 1.25/1).
  expect(screen.getAllByText('1.25×')).toHaveLength(2)

  // Bump milk 1.25 → 1.5.
  fireEvent.change(screen.getByDisplayValue('1.25'), {
    target: { value: '1.5' },
  })

  // Eggs row still shows 1.25×; milk row now shows 1.5×.
  await waitFor(() => {
    expect(screen.getByText('1.5×')).toBeInTheDocument()
    expect(screen.getByText('1.25×')).toBeInTheDocument()
  })
})

it('discards local edits when the user re-Previews with a different target', async () => {
  const parsed = makeParsed()

  // First Preview returns scale-factor 1.25 (eggs 3.75, milk 1.25).
  mockJson('POST', '/api/recipes/r1/scale', makeScaleResult())

  renderWithProviders(
    <ScaleRecipePanel
      parsed={parsed}
      onSaveAsNew={() => {}}
      onUpdateInPlace={() => {}}
      onCancel={() => {}}
    />,
  )

  fireEvent.change(screen.getByDisplayValue('4'), { target: { value: '5' } })
  fireEvent.click(screen.getByRole('button', { name: /preview/i }))

  await waitFor(() =>
    expect(screen.getByDisplayValue('1.25')).toBeInTheDocument()
  )

  // Edit milk locally to a deliberately weird value, confirm it stuck.
  fireEvent.change(screen.getByDisplayValue('1.25'), {
    target: { value: '7.7' },
  })
  expect(screen.getByDisplayValue('7.7')).toBeInTheDocument()

  // Re-Preview with a new target (4 -> 6). Second mock returns 1.5× scale.
  mockJson('POST', '/api/recipes/r1/scale', {
    ingredients: [
      { name: 'eggs', amount: { type: 'single', value: 4.5 }, unit: '' },
      { name: 'milk', amount: { type: 'single', value: 1.5 }, unit: 'cup' },
    ],
    flagged: [{ index: 0, name: 'eggs', scaled_value: 4.5, unit: '' }],
  })

  fireEvent.change(screen.getByDisplayValue('5'), { target: { value: '6' } })
  fireEvent.click(screen.getByRole('button', { name: /preview/i }))

  // The 7.7 edit should be gone; milk now shows 1.5 from the new preview.
  await waitFor(() =>
    expect(screen.getByDisplayValue('1.5')).toBeInTheDocument()
  )
  expect(screen.queryByDisplayValue('7.7')).not.toBeInTheDocument()
})
```

- [ ] **Step 2: Run the new tests, confirm they pass**

```bash
bun run test src/components/RecipeManager.test.tsx -- --run -t "ScaleRecipePanel"
```

Expected: both new tests pass (and the prior Task-2 test still passes).

- [ ] **Step 3: Commit**

```bash
git add src/components/RecipeManager.test.tsx
git commit -m "$(cat <<'EOF'
test(fewd-b3x): pin ratio update and re-Preview reset behavior

Two regression tests for the new Scale Recipe panel:
- editing one row updates only that row's ratio cell;
- clicking Preview with a new target discards local edits and replaces
  every row with the fresh backend response.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Full local CI gate

**Files:** none (verification only)

- [ ] **Step 1: Format**

```bash
dprint fmt
```

Expected: either "no changes" or a small set of formatting diffs.

- [ ] **Step 2: Lint**

```bash
bun run lint
```

Expected: clean (no errors, no warnings).

- [ ] **Step 3: Type-check (Vitest doesn't run tsc per CLAUDE.md)**

```bash
bunx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 4: Full test suite**

```bash
bun run test -- --run
```

Expected: all tests pass.

- [ ] **Step 5: Production build (catches tsc issues vitest misses)**

```bash
bun run build
```

Expected: build succeeds.

- [ ] **Step 6: If `dprint fmt` produced diffs, commit them**

```bash
git status                     # any unstaged changes?
git diff                       # review
# if there are diffs:
git add -u
git commit -m "$(cat <<'EOF'
style(fewd-b3x): apply dprint formatting

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Manual smoke test

**Files:** none (manual verification)

Per CLAUDE.md project rules: "For UI or frontend changes, start the dev server and use the feature in a browser before reporting the task as complete."

- [ ] **Step 1: Start the dev server**

```bash
bun run dev:full
```

Wait for server output: backend on port 3000, frontend on port 5173.

- [ ] **Step 2: In a browser, open `http://localhost:5173`. Navigate Recipes → pick any recipe → click "Scale Recipe" (the existing button on the recipe detail page).**

- [ ] **Step 3: Verify the new layout**

Check by eye:

- Each ingredient row shows three columns: Original (read-only, right-aligned) | Current (editable input) | Ratio (e.g. "1×" before changing servings).
- Column header row shows "Original / Current / Ratio".
- Row width hugs content rather than stretching the panel.
- No amber "fractional" warning banner anywhere.

- [ ] **Step 4: Verify behavior**

- Change target servings from N → N+1, click Preview. Every row updates and the Ratio column shows the new (e.g.) "1.25×".
- Edit a single row's input. That row's Ratio cell updates; others don't.
- Click Preview again with a different target. Your edits are discarded; rows show the new ratio.
- Click "Save as New Recipe" with some edits in place. Verify the new recipe is created with the displayed values (Recipes list updates, new recipe is visible).

- [ ] **Step 5: Stop the dev server (Ctrl-C in its terminal).**

---

## Task 6: Push, open PR, link the bead

- [ ] **Step 1: Push the branch**

```bash
git push -u origin fewd-b3x/ratio-column-free-edit
```

- [ ] **Step 2: Open PR**

```bash
gh pr create --title "feat(fewd-b3x): Scale Recipe panel — ratio column + free-form editing" --body "$(cat <<'EOF'
## Summary
- Replaces the post-Preview state of the Scale Recipe form with a 3-column grid: Original | Current (editable) | Ratio.
- Every row is editable, not just rows the backend flagged as fractional-discrete.
- New `ingredientRatio` + `formatRatio` helpers in `src/types/recipe.ts`.
- Removes the flagged-only highlight machinery (amber row background, "fractional" pill, `flaggedIndices` Set). Keeps the "fractional amounts" warning banner until `fewd-2bp` lands.

## Why
Today's panel forces the user to fix flagged values manually but doesn't propagate those edits to the rest of the recipe — and gives no visibility into how far the recipe has drifted from its original ratios. The Ratio column makes drift legible; free-form editing turns the post-Preview state into a general kitchen-adjustment surface.

The earlier draft of this spec proposed a clean-multiplier-hunting Rebalance machinery; a chef-domain reality check (real kitchens round and accept small drift, baker's percentages convert eggs to grams) made that over-engineered. Weight-based scaling is logged as a future pivot (fewd-bhd).

## Spec
`docs/superpowers/specs/2026-05-17-recipe-scaling-rebalance-and-fine-tune-design.md`

## Test plan
- [x] Unit tests for `ingredientRatio` / `formatRatio` (recipe.test.ts).
- [x] RTL tests for `ScaleRecipePanel`: all rows editable, ratio updates per-row, re-Preview discards local edits.
- [x] Full local CI gate: `dprint fmt`, `bun run lint`, `bunx tsc --noEmit`, `bun run test`, `bun run build`.
- [x] Manual smoke in the browser per CLAUDE.md "For UI or frontend changes…" rule.

## Related beads
- Composable with **fewd-2bp** (auto-round at Preview, should ship before or after — either order works).
- Closes **fewd-0mt** (flagged-row highlight bug; the highlight UI is removed entirely).
- See **fewd-bhd** (weight-based scaling, potential future pivot).

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 3: Report PR URL back to the user**

Once `gh pr create` prints the URL, report it. Do not close fewd-b3x yet — per CLAUDE.md, bead closure is a separate `chore(beads): close fewd-b3x after PR #N merge` commit on `main` AFTER the PR merges.

---

## Out of scope (for this plan)

These belong to other beads and must not be touched in this PR:

- Auto-rounding fractional discrete units at the scaling service layer → **fewd-2bp**.
- Closing the flagged-highlight bead `fewd-0mt` → separate `chore(beads): close fewd-0mt …` commit after this PR merges (since this PR makes the bug moot).
- Weight-based scaling for discrete ingredients → **fewd-bhd** (potential future pivot).
- Shopping-list rounding → **fewd-p3j** (different service entirely).
