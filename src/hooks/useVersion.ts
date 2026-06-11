import { useQuery } from '@tanstack/react-query'
import { api } from '../lib/api'
import type { VersionInfo } from '../types/version'

export function useVersion() {
  return useQuery({
    queryKey: ['version'],
    queryFn: () => api.get<VersionInfo>('/version'),
    // Baked in at build time — can only change when the server restarts on a
    // new binary, so never refetch within a page load.
    staleTime: Infinity,
    retry: false,
  })
}
