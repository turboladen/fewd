import type { Ingredient } from './recipe'

export interface DrinkRecipe {
  id: string
  slug: string
  name: string
  description: string | null
  source: string
  source_url: string | null
  servings: number
  instructions: string
  ingredients: string
  technique: string | null
  glassware: string | null
  garnish: string | null
  tags: string
  notes: string | null
  icon: string | null
  is_favorite: boolean
  is_non_alcoholic: boolean
  rating: number | null
  times_made: number
  created_at: string
  updated_at: string
}

export interface CreateDrinkRecipeDto {
  name: string
  description?: string
  source: string
  servings: number
  instructions: string
  ingredients: Ingredient[]
  technique?: string
  glassware?: string
  garnish?: string
  tags: string[]
  notes?: string
  icon?: string
  is_non_alcoholic?: boolean
}

export interface UpdateDrinkRecipeDto {
  name?: string
  description?: string
  servings?: number
  instructions?: string
  ingredients?: Ingredient[]
  technique?: string
  glassware?: string
  garnish?: string
  tags?: string[]
  notes?: string
  icon?: string
  is_favorite?: boolean
  is_non_alcoholic?: boolean
  rating?: number
}

export interface ParsedDrinkRecipe extends Omit<DrinkRecipe, 'ingredients' | 'tags'> {
  ingredients: Ingredient[]
  tags: string[]
}

export function parseDrinkRecipe(recipe: DrinkRecipe): ParsedDrinkRecipe {
  return {
    ...recipe,
    ingredients: JSON.parse(recipe.ingredients) as Ingredient[],
    tags: JSON.parse(recipe.tags) as string[],
  }
}

export interface DrinkRecipeFormData {
  name: string
  description: string
  icon: string
  servings: number
  instructions: string
  ingredients: Ingredient[]
  technique: string
  glassware: string
  garnish: string
  tags: string[]
  notes: string
  is_non_alcoholic: boolean
}

export const emptyDrinkRecipeForm: DrinkRecipeFormData = {
  name: '',
  description: '',
  icon: '',
  servings: 1,
  instructions: '',
  ingredients: [],
  technique: '',
  glassware: '',
  garnish: '',
  tags: [],
  notes: '',
  is_non_alcoholic: false,
}

export type DrinkMood =
  | { type: 'style'; label: string }
  | { type: 'custom'; text: string }

export type SuggestionSource = 'both' | 'ai-only' | 'recipes-only'

export interface AiSuggestCocktailsDto {
  person_ids: string[]
  bar_item_ids: string[]
  mood: DrinkMood
  include_non_alcoholic: boolean
  feedback?: string
  previous_suggestion_names?: string[]
}
