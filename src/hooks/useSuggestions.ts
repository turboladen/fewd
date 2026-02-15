import { useMutation } from '@tanstack/react-query'
import { api } from '../lib/api'
import type { CreateRecipeDto } from '../types/recipe'
import type { AiSuggestMealsDto, GetSuggestionsDto, MealSuggestions } from '../types/suggestion'
import { useStreamingMutation } from './useStreamingMutation'

export function useMealSuggestions() {
  return useMutation({
    mutationFn: (data: GetSuggestionsDto) => api.post<MealSuggestions>('/suggestions', data),
  })
}

export function useAiSuggestMeals() {
  return useStreamingMutation<AiSuggestMealsDto, CreateRecipeDto[]>({
    path: '/suggestions/ai',
  })
}
