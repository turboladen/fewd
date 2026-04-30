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
  times_made: number
  last_made: string | null
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

  const stripMarker = (chunk: string) => chunk.replace(/^\s*\d+[.)]\s*/, '').trim()
  const joinSoftWrap = (chunk: string) => chunk.replace(/\s*\n\s*/g, ' ').trim()
  const finalize = (chunks: string[]) =>
    chunks
      .map(joinSoftWrap)
      .map(stripMarker)
      .filter((s) => s.length > 0)

  if (/\n[ \t]*\n/.test(trimmed)) {
    return finalize(trimmed.split(/\n[ \t]*\n+/))
  }

  const markerCount = (trimmed.match(/(?:^|\n)[ \t]*\d+[.)][ \t]+/g) ?? []).length
  if (markerCount >= 2) {
    return finalize(trimmed.split(/\n(?=[ \t]*\d+[.)][ \t]+)/))
  }

  return trimmed
    .split('\n')
    .map(stripMarker)
    .filter((s) => s.length > 0)
}
