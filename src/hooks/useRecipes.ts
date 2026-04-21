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
import { useStreamingMutation } from './useStreamingMutation'

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

/**
 * Cached fetch of the AI-enhanced instructions for a recipe. Used by cook
 * mode to auto-load the enhanced text once per session per recipe; the
 * imperative `useEnhanceInstructions` mutation above continues to drive
 * the user-toggled "Enhanced view" affordance on the detail page.
 */
export function useEnhancedInstructions(id: string, enabled: boolean) {
  return useQuery({
    queryKey: ['recipes', id, 'enhanced'],
    queryFn: () => api.post<string>('/recipes/' + id + '/enhance'),
    enabled: enabled && !!id,
    staleTime: Infinity,
    retry: false,
  })
}

export function useAdaptRecipe() {
  return useStreamingMutation<AdaptRecipeDto, CreateRecipeDto>({
    path: (data) => '/recipes/' + data.recipe_id + '/adapt',
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
  return useStreamingMutation<{ url: string }, Recipe>({
    path: '/recipes/import/url',
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
