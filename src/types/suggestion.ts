import type { PersonAdaptOptions } from './recipe'

export interface SuggestionItem {
  recipe_id: string
  recipe_name: string
  rating: number | null
  last_made: string | null
  times_made: number
  reason: string
}

export interface MealSuggestions {
  recent_favorites: SuggestionItem[]
  forgotten_hits: SuggestionItem[]
  untried: SuggestionItem[]
}

export interface GetSuggestionsDto {
  person_ids: string[]
  reference_date: string
}

export type MealCharacter =
  | { type: 'balanced' }
  | { type: 'indulgent' }
  | { type: 'quick' }
  | { type: 'custom'; text: string }

export interface AiSuggestMealsDto {
  person_options: PersonAdaptOptions[]
  meal_type: string
  character: MealCharacter
  feedback?: string
  previous_suggestion_names?: string[]
}
