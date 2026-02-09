import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { invoke } from '@tauri-apps/api/core'
import type { ModelOption, TokenUsage } from '../types/settings'

export function useSetting(key: string) {
  return useQuery({
    queryKey: ['settings', key],
    queryFn: () => invoke<string | null>('get_setting', { key }),
  })
}

export function useSetSetting() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({ key, value }: { key: string; value: string }) =>
      invoke('set_setting', { key, value }),
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
    queryFn: () => invoke<ModelOption[]>('get_available_models'),
  })
}

export function useTestConnection() {
  return useMutation({
    mutationFn: () => invoke<string>('test_claude_connection'),
  })
}

export function useTokenUsage() {
  return useQuery({
    queryKey: ['token-usage'],
    queryFn: () => invoke<TokenUsage>('get_token_usage'),
  })
}
