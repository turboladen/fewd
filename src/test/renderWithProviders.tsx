import { type QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, type RenderOptions, type RenderResult } from '@testing-library/react'
import type { ReactElement, ReactNode } from 'react'
import { MemoryRouter } from 'react-router-dom'
import { ToastProvider } from '../components/Toast'
import { ChromeProvider } from '../contexts/ChromeContext'
import { createTestQueryClient } from './queryClient'

interface Options extends Omit<RenderOptions, 'wrapper'> {
  client?: QueryClient
  initialPath?: string
}

/**
 * Render a component with the full app provider stack: TanStack Query,
 * React Router (MemoryRouter), and Toast. Returns the standard RTL result
 * plus the QueryClient so tests can spy on `invalidateQueries`.
 */
export function renderWithProviders(
  ui: ReactElement,
  options: Options = {},
): RenderResult & { client: QueryClient } {
  const client = options.client ?? createTestQueryClient()
  const { initialPath, ...rtl } = options
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[initialPath ?? '/']}>
        <ToastProvider>
          <ChromeProvider>{children}</ChromeProvider>
        </ToastProvider>
      </MemoryRouter>
    </QueryClientProvider>
  )
  return { ...render(ui, { wrapper: Wrapper, ...rtl }), client }
}
