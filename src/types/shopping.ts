import type { IngredientAmount } from './recipe'

export interface IngredientSource {
  amount: IngredientAmount
  unit: string
  source_type: 'recipe' | 'adhoc'
  source_name: string | null
  meal_id: string
  meal_date: string
  meal_type: string
}

export interface AggregatedIngredient {
  ingredient_name: string
  total_amount: IngredientAmount | null
  total_unit: string | null
  items: IngredientSource[]
}
