import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '../lib/api'
import type {
  AdaptRecipeDto,
  CreateRecipeDto,
  ImportRecipeDto,
  Recipe,
  ScaleResult,
  UpdateRecipeDto,
} from '../types/recipe'

export function useRecipes() {
  return useQuery({
    queryKey: ['recipes'],
    queryFn: () => api.get<Recipe[]>('/recipes'),
  })
}

export function useRecipe(id: string) {
  return useQuery({
    queryKey: ['recipes', id],
    queryFn: () => api.get<Recipe | null>('/recipes/' + id),
    enabled: !!id,
  })
}

export function useSearchRecipes(query: string) {
  return useQuery({
    queryKey: ['recipes', 'search', query],
    queryFn: () => api.get<Recipe[]>('/recipes/search?q=' + encodeURIComponent(query)),
    enabled: query.length > 0,
  })
}

export function useCreateRecipe() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: CreateRecipeDto) => api.post<Recipe>('/recipes', data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recipes'] })
    },
  })
}

export function useUpdateRecipe() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateRecipeDto }) =>
      api.put<Recipe>('/recipes/' + id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recipes'] })
    },
  })
}

export function useDeleteRecipe() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => api.delete('/recipes/' + id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recipes'] })
    },
  })
}

export function useToggleFavorite() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => api.post<Recipe>('/recipes/' + id + '/favorite'),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recipes'] })
    },
  })
}

export function usePreviewScaleRecipe() {
  return useMutation({
    mutationFn: ({ id, newServings }: { id: string; newServings: number }) =>
      api.post<ScaleResult>('/recipes/' + id + '/scale', { new_servings: newServings }),
  })
}

export function useEnhanceInstructions() {
  return useMutation({
    mutationFn: (id: string) => api.post<string>('/recipes/' + id + '/enhance'),
  })
}

export function useAdaptRecipe() {
  return useMutation({
    mutationFn: (data: AdaptRecipeDto) =>
      api.post<CreateRecipeDto>('/recipes/' + data.recipe_id + '/adapt', data),
  })
}

export function useImportRecipe() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: ImportRecipeDto) => api.post<Recipe>('/recipes/import/markdown', data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recipes'] })
    },
  })
}

export function useImportRecipeFromUrl() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: { url: string }) => api.post<Recipe>('/recipes/import/url', data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recipes'] })
    },
  })
}

export function useImportRecipeFromFile() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (file: File) => api.upload<Recipe>('/recipes/import/file', file),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recipes'] })
    },
  })
}
