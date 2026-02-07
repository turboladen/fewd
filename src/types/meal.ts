import type { Ingredient } from './recipe'

export type PersonServing =
  | {
    food_type: 'recipe'
    person_id: string
    recipe_id: string
    servings_count: number
    notes: string | null
  }
  | {
    food_type: 'adhoc'
    person_id: string
    adhoc_items: Ingredient[]
    notes: string | null
  }

export interface Meal {
  id: string
  date: string
  meal_type: string
  order_index: number
  servings: string // JSON string of PersonServing[]
  created_at: string
  updated_at: string
}

export interface ParsedMeal extends Omit<Meal, 'servings'> {
  servings: PersonServing[]
}

export interface CreateMealDto {
  date: string
  meal_type: string
  order_index: number
  servings: PersonServing[]
}

export interface UpdateMealDto {
  date?: string
  meal_type?: string
  order_index?: number
  servings?: PersonServing[]
}

export function parseMeal(meal: Meal): ParsedMeal {
  return {
    ...meal,
    servings: JSON.parse(meal.servings) as PersonServing[],
  }
}
