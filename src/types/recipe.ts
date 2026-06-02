export interface TimeValue {
  value: number
  unit: 'minutes' | 'hours' | 'days'
}

export interface PortionSize {
  value: number
  unit: string
}

export type IngredientAmount =
  | { type: 'single'; value: number }
  | { type: 'range'; min: number; max: number }

export interface Ingredient {
  /**
   * Purchasable identity (e.g. "garlic"). Distinct varietals like "boneless
   * skinless chicken breast" vs "whole chicken" stay as separate names — the
   * shopping list aggregates by this field.
   */
  name: string
  /**
   * Optional preparation form (e.g. "minced", "thinly sliced"). The shopping
   * list ignores this; it belongs to the recipe step.
   */
  prep?: string
  amount: IngredientAmount
  unit: string
  notes?: string
  /**
   * Optional alternative ingredient parsed from `<primary> or <alt>` lines
   * (e.g. "8 flour tortillas or 10 corn tortillas"). Recursive — the
   * alternative carries its own amount/unit/prep/notes. Snake_case matches
   * the Rust DTO's wire format (no `rename_all` applied server-side).
   */
  or_alternative?: Ingredient
}

/**
 * Trim a prep value and coerce empty / whitespace-only strings to undefined.
 * Mirrors the backend's `deserialize_optional_string_empty_as_none` rule so
 * the frontend's notion of "absent prep" matches what the server stores and
 * what the shopping aggregator keys on.
 */
export function normalizeIngredientPrep(prep?: string): string | undefined {
  if (prep === undefined) return undefined
  const trimmed = prep.trim()
  return trimmed.length > 0 ? trimmed : undefined
}

/**
 * Compose the ingredient label as `{name}, {prep}` when prep is present,
 * otherwise just `{name}`. Use this everywhere a recipe ingredient is
 * displayed so the rule stays consistent.
 */
export function formatIngredientLabel(ing: Ingredient): string {
  const prep = normalizeIngredientPrep(ing.prep)
  return prep ? `${ing.name}, ${prep}` : ing.name
}

export interface Nutrition {
  calories?: number
  protein_grams?: number
  carbs_grams?: number
  fat_grams?: number
  notes?: string
}

export interface Recipe {
  id: string
  slug: string
  name: string
  description: string | null
  source: string
  source_url: string | null
  parent_recipe_id: string | null
  /** Resolved server-side on single-recipe GETs when parent_recipe_id is set. */
  parent_name?: string | null
  parent_slug?: string | null
  prep_time: string | null
  cook_time: string | null
  total_time: string | null
  servings: number
  portion_size: string | null
  instructions: string
  ingredients: string
  nutrition_per_serving: string | null
  tags: string
  notes: string | null
  icon: string | null
  is_favorite: boolean
  times_planned: number
  last_planned: string | null
  rating: number | null
  created_at: string
  updated_at: string
}

export interface CreateRecipeDto {
  name: string
  description?: string
  source: string
  parent_recipe_id?: string
  prep_time?: TimeValue
  cook_time?: TimeValue
  total_time?: TimeValue
  servings: number
  portion_size?: PortionSize
  instructions: string
  ingredients: Ingredient[]
  nutrition_per_serving?: Nutrition
  tags: string[]
  notes?: string
  icon?: string
}

export interface UpdateRecipeDto {
  name?: string
  description?: string
  prep_time?: TimeValue
  cook_time?: TimeValue
  total_time?: TimeValue
  servings?: number
  portion_size?: PortionSize
  instructions?: string
  ingredients?: Ingredient[]
  nutrition_per_serving?: Nutrition
  tags?: string[]
  notes?: string
  icon?: string
  is_favorite?: boolean
  rating?: number
}

export interface ImportRecipeDto {
  markdown: string
}

export interface FlaggedIngredient {
  index: number
  name: string
  scaled_value: number
  unit: string
}

export interface ScaleResult {
  ingredients: Ingredient[]
  flagged: FlaggedIngredient[]
}

// --- AI Adaptation ---

export interface PersonAdaptOptions {
  person_id: string
  include_dietary_goals: boolean
  include_dislikes: boolean
  include_favorites: boolean
}

export interface AdaptRecipeDto {
  recipe_id: string
  person_options: PersonAdaptOptions[]
  user_instructions: string
}

export interface ParsedRecipe extends
  Omit<
    Recipe,
    | 'prep_time'
    | 'cook_time'
    | 'total_time'
    | 'portion_size'
    | 'ingredients'
    | 'nutrition_per_serving'
    | 'tags'
  >
{
  prep_time: TimeValue | null
  cook_time: TimeValue | null
  total_time: TimeValue | null
  portion_size: PortionSize | null
  ingredients: Ingredient[]
  nutrition_per_serving: Nutrition | null
  tags: string[]
}

export function parseRecipe(recipe: Recipe): ParsedRecipe {
  return {
    ...recipe,
    prep_time: recipe.prep_time ? JSON.parse(recipe.prep_time) as TimeValue : null,
    cook_time: recipe.cook_time ? JSON.parse(recipe.cook_time) as TimeValue : null,
    total_time: recipe.total_time ? JSON.parse(recipe.total_time) as TimeValue : null,
    portion_size: recipe.portion_size ? JSON.parse(recipe.portion_size) as PortionSize : null,
    ingredients: JSON.parse(recipe.ingredients) as Ingredient[],
    nutrition_per_serving: recipe.nutrition_per_serving
      ? JSON.parse(recipe.nutrition_per_serving) as Nutrition
      : null,
    tags: JSON.parse(recipe.tags) as string[],
  }
}

export function formatTime(time: TimeValue | null): string {
  if (!time) return ''
  return `${time.value} ${time.unit}`
}

export function formatServings(servings: number, portionSize: PortionSize | null): string {
  if (!portionSize) return `${servings}`
  return `${servings} (${portionSize.value} ${portionSize.unit} each)`
}

export function formatAmount(amount: IngredientAmount): string {
  if (amount.type === 'single') {
    return amount.value % 1 === 0
      ? String(amount.value)
      : amount.value.toFixed(2).replace(/0+$/, '').replace(/\.$/, '')
  }
  return `${amount.min}-${amount.max}`
}

/**
 * Returns `current / original`, or `null` if the original reference is 0.
 * For `range` amounts on either side, uses `.min` on both — callers that
 * need full-range ratios should compute them separately.
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

/**
 * Splits free-form instructions into trimmed, non-empty steps. Leading list
 * markers like `1.` or `2)` are stripped so the renderer can supply its own
 * numbering.
 *
 * Three strategies are tried in order so AI-enhanced prose (hard-wrapped at
 * ~80 chars within paragraphs) and user-typed lists both render correctly:
 *
 *   1. If the input contains a blank line, paragraphs (separated by `\n\n+`)
 *      are the steps; soft-wrapped lines within a paragraph are joined with
 *      a space.
 *   2. Else, if 2+ lines start with a numbered marker (`1.`, `2)`, …), split
 *      on those markers so a wrapped item like
 *      `1. Heat oil.\n   Add onions.\n2. Add garlic.` becomes 2 steps.
 *   3. Else, fall back to one step per non-empty line — preserves how
 *      user-typed multi-line instructions have always rendered.
 */
export function parseInstructionSteps(instructions: string): string[] {
  const trimmed = instructions.replace(/\r\n?/g, '\n').trim()
  if (trimmed.length === 0) return []

  const markerBoundary = /\n(?=[ \t]*\d+[.)][ \t]+)/
  const countMarkers = (text: string) => (text.match(/(?:^|\n)[ \t]*\d+[.)][ \t]+/g) ?? []).length
  // Strip a leading list marker — numbered (`1.`/`2)`) or bullet (`-`/`*`/`+`) —
  // so the renderer supplies its own numbering. The trailing `\s+` (not `\s*`)
  // requires whitespace after the marker, so a decimal quantity like "1.5 cups"
  // is left intact rather than parsed as a "1." list marker.
  const stripMarker = (chunk: string) => chunk.replace(/^\s*(?:\d+[.)]|[-*+])\s+/, '').trim()
  const joinSoftWrap = (chunk: string) => chunk.replace(/\s*\n\s*/g, ' ').trim()
  // A blank-line "paragraph" may itself hold a contiguous numbered list (no
  // blanks between items) — split those out so each numbered step stays distinct.
  const splitByMarkers = (chunk: string) =>
    countMarkers(chunk) >= 2 ? chunk.split(markerBoundary) : [chunk]
  const finalize = (chunks: string[]) =>
    chunks
      .map(joinSoftWrap)
      .map(stripMarker)
      .filter((s) => s.length > 0)

  if (/\n[ \t]*\n/.test(trimmed)) {
    return finalize(trimmed.split(/\n[ \t]*\n+/).flatMap(splitByMarkers))
  }

  if (countMarkers(trimmed) >= 2) {
    return finalize(trimmed.split(markerBoundary))
  }

  return trimmed
    .split('\n')
    .map(stripMarker)
    .filter((s) => s.length > 0)
}

/** A markdown `##` section heading paired with the steps that follow it. */
export interface InstructionSection {
  /** Heading text (markers stripped), or `null` for steps before the first heading. */
  heading: string | null
  steps: string[]
}

/**
 * Group instructions into sections by markdown headings (`#`–`######`) for
 * cook mode, so sub-recipe components ("## Custard Base") render as section
 * dividers between step-card groups instead of leaking literal `##` into a step.
 *
 * Each section's body is segmented by {@link parseInstructionSteps}, so steps
 * split (and numbered markers strip) exactly as in a single-section recipe.
 * Steps before any heading become a leading section with `heading: null`.
 * Only sections that actually contain steps are emitted — a heading with no
 * body would otherwise render a divider above an empty list in cook mode.
 */
export function parseInstructionSections(instructions: string): InstructionSection[] {
  const headingRe = /^[ \t]*#{1,6}[ \t]+(.*)$/
  const sections: InstructionSection[] = []
  let heading: string | null = null
  let body: string[] = []

  const flush = () => {
    const steps = parseInstructionSteps(body.join('\n'))
    if (steps.length > 0) {
      sections.push({ heading, steps })
    }
    body = []
  }

  for (const line of instructions.replace(/\r\n?/g, '\n').split('\n')) {
    const match = line.match(headingRe)
    if (match) {
      flush()
      // Strip the optional ATX closing sequence ("## Base ##" -> "Base"), and
      // treat a bare `## ` with no title as no heading rather than `heading: ''`.
      heading = match[1].replace(/[ \t]+#+[ \t]*$/, '').trim() || null
    } else {
      body.push(line)
    }
  }
  flush()

  return sections
}
