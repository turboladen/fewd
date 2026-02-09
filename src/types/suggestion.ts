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
