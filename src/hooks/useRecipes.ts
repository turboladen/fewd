import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { invoke } from '@tauri-apps/api/core'
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
    queryFn: () => invoke<Recipe[]>('get_all_recipes'),
  })
}

export function useRecipe(id: string) {
  return useQuery({
    queryKey: ['recipes', id],
    queryFn: () => invoke<Recipe | null>('get_recipe', { id }),
    enabled: !!id,
  })
}

export function useSearchRecipes(query: string) {
  return useQuery({
    queryKey: ['recipes', 'search', query],
    queryFn: () => invoke<Recipe[]>('search_recipes', { query }),
    enabled: query.length > 0,
  })
}

export function useCreateRecipe() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: CreateRecipeDto) => invoke<Recipe>('create_recipe', { data }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recipes'] })
    },
  })
}

export function useUpdateRecipe() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateRecipeDto }) =>
      invoke<Recipe>('update_recipe', { id, data }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recipes'] })
    },
  })
}

export function useDeleteRecipe() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => invoke('delete_recipe', { id }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recipes'] })
    },
  })
}

export function useToggleFavorite() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => invoke<Recipe>('toggle_favorite_recipe', { id }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recipes'] })
    },
  })
}

export function usePreviewScaleRecipe() {
  return useMutation({
    mutationFn: ({ id, newServings }: { id: string; newServings: number }) =>
      invoke<ScaleResult>('preview_scale_recipe', { id, newServings }),
  })
}

export function useEnhanceInstructions() {
  return useMutation({
    mutationFn: (id: string) => invoke<string>('enhance_recipe_instructions', { id }),
  })
}

export function useAdaptRecipe() {
  return useMutation({
    mutationFn: (data: AdaptRecipeDto) => invoke<CreateRecipeDto>('adapt_recipe', { data }),
  })
}

export function useImportRecipe() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: ImportRecipeDto) => invoke<Recipe>('import_recipe_from_markdown', { data }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recipes'] })
    },
  })
}
