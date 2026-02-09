import { useMutation } from '@tanstack/react-query'
import { invoke } from '@tauri-apps/api/core'
import type { GetSuggestionsDto, MealSuggestions } from '../types/suggestion'

export function useMealSuggestions() {
  return useMutation({
    mutationFn: (data: GetSuggestionsDto) =>
      invoke<MealSuggestions>('get_meal_suggestions', { data }),
  })
}
