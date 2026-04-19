import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'

/**
 * Test wrapper for TanStack Query hooks.
 *
 * @example
 *   const { Wrapper, client } = createQueryWrapper()
 *   const { result } = renderHook(() => usePeople(), { wrapper: Wrapper })
 *   await waitFor(() => expect(result.current.isSuccess).toBe(true))
 *
 * Pass the same wrapper to two renderHook calls to share cache state
 * (needed for testing mutation → invalidation → refetch flows).
 */
export function createTestQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: Infinity, staleTime: 0 },
      mutations: { retry: false },
    },
  })
}

export function createQueryWrapper(client: QueryClient = createTestQueryClient()) {
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
  return { Wrapper, client }
}
