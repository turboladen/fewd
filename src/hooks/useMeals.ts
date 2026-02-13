import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '../lib/api'
import type { CreateMealDto, Meal, UpdateMealDto } from '../types/meal'

export function useMealsForDateRange(startDate: string, endDate: string) {
  return useQuery({
    queryKey: ['meals', 'date-range', startDate, endDate],
    queryFn: () => api.get<Meal[]>('/meals?start_date=' + startDate + '&end_date=' + endDate),
    enabled: !!startDate && !!endDate,
  })
}

export function useMeal(id: string) {
  return useQuery({
    queryKey: ['meals', id],
    queryFn: () => api.get<Meal | null>('/meals/' + id),
    enabled: !!id,
  })
}

export function useCreateMeal() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: CreateMealDto) => api.post<Meal>('/meals', data),
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
      api.put<Meal>('/meals/' + id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['meals'] })
      queryClient.invalidateQueries({ queryKey: ['recipes'] })
    },
  })
}

export function useDeleteMeal() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => api.delete('/meals/' + id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['meals'] })
    },
  })
}
