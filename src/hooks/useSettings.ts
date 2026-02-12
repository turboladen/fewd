import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '../lib/api'
import type {
  ModelOption,
  TokenUsage,
} from '../types/settings'

export function useSetting(key: string) {
  return useQuery({
    queryKey: ['settings', key],
    queryFn: () => api.get<string | null>('/settings/' + key),
  })
}

export function useSetSetting() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({ key, value }: { key: string; value: string }) =>
      api.put('/settings/' + key, { value }),
    onSuccess: (_data, { key }) => {
      queryClient.invalidateQueries({ queryKey: ['settings', key] })
      // Refresh models list when API key changes (models are fetched dynamically)
      if (key === 'anthropic_api_key') {
        queryClient.invalidateQueries({ queryKey: ['available-models'] })
      }
    },
  })
}

export function useAvailableModels() {
  return useQuery({
    queryKey: ['available-models'],
    queryFn: () => api.get<ModelOption[]>('/settings/models'),
  })
}

export function useTestConnection() {
  return useMutation({
    mutationFn: () => api.post<string>('/settings/test-connection'),
  })
}

export function useTokenUsage() {
  return useQuery({
    queryKey: ['token-usage'],
    queryFn: () => api.get<TokenUsage>('/settings/token-usage'),
  })
}
