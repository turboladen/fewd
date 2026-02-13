import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '../lib/api'
import type {
  CreateFromMealDto,
  CreateMealTemplateDto,
  MealTemplate,
  UpdateMealTemplateDto,
} from '../types/mealTemplate'

export function useMealTemplates() {
  return useQuery({
    queryKey: ['meal_templates'],
    queryFn: () => api.get<MealTemplate[]>('/meal-templates'),
  })
}

export function useCreateMealTemplate() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: CreateMealTemplateDto) => api.post<MealTemplate>('/meal-templates', data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['meal_templates'] })
    },
  })
}

export function useUpdateMealTemplate() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateMealTemplateDto }) =>
      api.put<MealTemplate>('/meal-templates/' + id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['meal_templates'] })
    },
  })
}

export function useDeleteMealTemplate() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => api.delete('/meal-templates/' + id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['meal_templates'] })
    },
  })
}

export function useCreateTemplateFromMeal() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: CreateFromMealDto) =>
      api.post<MealTemplate>('/meal-templates/from-meal', data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['meal_templates'] })
    },
  })
}
