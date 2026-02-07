import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { invoke } from '@tauri-apps/api/core'
import type { CreatePersonDto, Person, UpdatePersonDto } from '../types/person'

export function usePeople() {
  return useQuery({
    queryKey: ['people'],
    queryFn: () => invoke<Person[]>('get_all_people'),
  })
}

export function usePerson(id: string) {
  return useQuery({
    queryKey: ['people', id],
    queryFn: () => invoke<Person | null>('get_person', { id }),
    enabled: !!id,
  })
}

export function useCreatePerson() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (data: CreatePersonDto) => invoke<Person>('create_person', { data }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['people'] })
    },
  })
}

export function useUpdatePerson() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdatePersonDto }) =>
      invoke<Person>('update_person', { id, data }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['people'] })
    },
  })
}

export function useDeletePerson() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => invoke('delete_person', { id }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['people'] })
    },
  })
}
