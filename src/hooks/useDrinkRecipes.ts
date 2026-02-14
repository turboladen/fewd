import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '../lib/api'
import type { CreateDrinkRecipeDto, DrinkRecipe, UpdateDrinkRecipeDto } from '../types/drinkRecipe'

export function useDrinkRecipes() {
  return useQuery({
    queryKey: ['drink-recipes'],
    queryFn: () => api.get<DrinkRecipe[]>('/drink-recipes'),
  })
}

export function useDrinkRecipe(id: string) {
  return useQuery({
    queryKey: ['drink-recipes', id],
    queryFn: () => api.get<DrinkRecipe>('/drink-recipes/' + id),
    enabled: !!id,
  })
}

export function useCreateDrinkRecipe() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: CreateDrinkRecipeDto) => api.post<DrinkRecipe>('/drink-recipes', data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['drink-recipes'] })
    },
  })
}

export function useUpdateDrinkRecipe() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateDrinkRecipeDto }) =>
      api.put<DrinkRecipe>('/drink-recipes/' + id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['drink-recipes'] })
    },
  })
}

export function useDeleteDrinkRecipe() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => api.delete('/drink-recipes/' + id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['drink-recipes'] })
    },
  })
}

export function useImportDrinkRecipeFromUrl() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: { url: string }) => api.post<DrinkRecipe>('/drink-recipes/import/url', data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['drink-recipes'] })
    },
  })
}

export function useToggleDrinkFavorite() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => api.post<DrinkRecipe>('/drink-recipes/' + id + '/favorite'),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['drink-recipes'] })
    },
  })
}
