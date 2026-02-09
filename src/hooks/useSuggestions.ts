import { useMutation } from '@tanstack/react-query'
import { invoke } from '@tauri-apps/api/core'
import type { CreateRecipeDto } from '../types/recipe'
import type { AiSuggestMealsDto, GetSuggestionsDto, MealSuggestions } from '../types/suggestion'

export function useMealSuggestions() {
  return useMutation({
    mutationFn: (data: GetSuggestionsDto) =>
      invoke<MealSuggestions>('get_meal_suggestions', { data }),
  })
}

export function useAiSuggestMeals() {
  return useMutation({
    mutationFn: (data: AiSuggestMealsDto) =>
      invoke<CreateRecipeDto[]>('ai_suggest_meals', { data }),
  })
}
