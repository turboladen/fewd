import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { invoke } from '@tauri-apps/api/core'
import type {
  CreateFromMealDto,
  CreateMealTemplateDto,
  MealTemplate,
  UpdateMealTemplateDto,
} from '../types/mealTemplate'

export function useMealTemplates() {
  return useQuery({
    queryKey: ['meal_templates'],
    queryFn: () => invoke<MealTemplate[]>('get_all_meal_templates'),
  })
}

export function useCreateMealTemplate() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: CreateMealTemplateDto) =>
      invoke<MealTemplate>('create_meal_template', { data }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['meal_templates'] })
    },
  })
}

export function useUpdateMealTemplate() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateMealTemplateDto }) =>
      invoke<MealTemplate>('update_meal_template', { id, data }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['meal_templates'] })
    },
  })
}

export function useDeleteMealTemplate() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => invoke('delete_meal_template', { id }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['meal_templates'] })
    },
  })
}

export function useCreateTemplateFromMeal() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: CreateFromMealDto) =>
      invoke<MealTemplate>('create_template_from_meal', { data }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['meal_templates'] })
    },
  })
}
