import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { invoke } from '@tauri-apps/api/core'
import type { CreateMealDto, Meal, UpdateMealDto } from '../types/meal'

export function useMealsForDateRange(startDate: string, endDate: string) {
  return useQuery({
    queryKey: ['meals', 'date-range', startDate, endDate],
    queryFn: () => invoke<Meal[]>('get_meals_for_date_range', { startDate, endDate }),
    enabled: !!startDate && !!endDate,
  })
}

export function useMeal(id: string) {
  return useQuery({
    queryKey: ['meals', id],
    queryFn: () => invoke<Meal | null>('get_meal', { id }),
    enabled: !!id,
  })
}

export function useCreateMeal() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: CreateMealDto) => invoke<Meal>('create_meal', { data }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['meals'] })
      queryClient.invalidateQueries({ queryKey: ['recipes'] })
    },
  })
}

export function useUpdateMeal() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateMealDto }) =>
      invoke<Meal>('update_meal', { id, data }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['meals'] })
      queryClient.invalidateQueries({ queryKey: ['recipes'] })
    },
  })
}

export function useDeleteMeal() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => invoke('delete_meal', { id }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['meals'] })
    },
  })
}
