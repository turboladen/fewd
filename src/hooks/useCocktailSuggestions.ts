import { useMutation } from '@tanstack/react-query'
import { api } from '../lib/api'
import type { AiSuggestCocktailsDto, CreateDrinkRecipeDto } from '../types/drinkRecipe'

export function useAiSuggestCocktails() {
  return useMutation({
    mutationFn: (data: AiSuggestCocktailsDto) =>
      api.post<CreateDrinkRecipeDto[]>('/cocktails/suggest', data),
  })
}
