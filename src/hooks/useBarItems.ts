import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '../lib/api'
import type { BarItem, BulkBarItemsDto, CreateBarItemDto } from '../types/barItem'

export function useBarItems() {
  return useQuery({
    queryKey: ['bar-items'],
    queryFn: () => api.get<BarItem[]>('/bar-items'),
  })
}

export function useCreateBarItem() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: CreateBarItemDto) => api.post<BarItem>('/bar-items', data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bar-items'] })
    },
  })
}

export function useBulkCreateBarItems() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: BulkBarItemsDto) => api.post<BarItem[]>('/bar-items/bulk', data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bar-items'] })
    },
  })
}

export function useDeleteBarItem() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => api.delete('/bar-items/' + id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bar-items'] })
    },
  })
}

export function useClearBarItems() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: () => api.delete('/bar-items/all'),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bar-items'] })
    },
  })
}
