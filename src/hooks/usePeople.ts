import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '../lib/api'
import type { CreatePersonDto, Person, UpdatePersonDto } from '../types/person'

export function usePeople() {
  return useQuery({
    queryKey: ['people'],
    queryFn: () => api.get<Person[]>('/people'),
  })
}

export function usePerson(id: string) {
  return useQuery({
    queryKey: ['people', id],
    queryFn: () => api.get<Person | null>('/people/' + id),
    enabled: !!id,
  })
}

export function useCreatePerson() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: CreatePersonDto) => api.post<Person>('/people', data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['people'] })
    },
  })
}

export function useUpdatePerson() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdatePersonDto }) =>
      api.put<Person>('/people/' + id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['people'] })
    },
  })
}

export function useDeletePerson() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => api.delete('/people/' + id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['people'] })
    },
  })
}
