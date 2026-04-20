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
  name: string
  amount: IngredientAmount
  unit: string
  notes?: string
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
 * Splits free-form instructions text into one trimmed, non-empty step per line.
 * Leading list markers like `1.` or `2)` are stripped so the renderer can
 * supply its own numbering.
 */
export function parseInstructionSteps(instructions: string): string[] {
  return instructions
    .split('\n')
    .map((line) => line.replace(/^\s*\d+[.)]\s*/, '').trim())
    .filter((line) => line.length > 0)
}
