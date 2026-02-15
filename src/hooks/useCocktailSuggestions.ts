import type { AiSuggestCocktailsDto, CreateDrinkRecipeDto } from '../types/drinkRecipe'
import { useStreamingMutation } from './useStreamingMutation'

export function useAiSuggestCocktails() {
  return useStreamingMutation<AiSuggestCocktailsDto, CreateDrinkRecipeDto[]>({
    path: '/cocktails/suggest',
  })
}
