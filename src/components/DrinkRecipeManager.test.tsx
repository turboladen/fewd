import { QueryClientProvider } from '@tanstack/react-query'
import { fireEvent, renderHook, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useDeleteDrinkRecipe } from '../hooks/useDrinkRecipes'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { renderWithProviders } from '../test/renderWithProviders'
import type { DrinkRecipe } from '../types/drinkRecipe'
import { DrinkRecipeManager } from './DrinkRecipeManager'

function makeDrink(overrides: Partial<DrinkRecipe> = {}): DrinkRecipe {
  return {
    id: 'd1',
    name: 'Old Fashioned',
    description: null,
    source: 'manual',
    source_url: null,
    servings: 1,
    instructions: 'Stir with ice.',
    ingredients: '[]',
    technique: null,
    glassware: null,
    garnish: null,
    tags: '[]',
    notes: null,
    icon: null,
    is_favorite: false,
    is_non_alcoholic: false,
    rating: null,
    times_made: 0,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
    ...overrides,
  }
}

beforeEach(() => installFetchMock())
afterEach(() => resetFetchMock())

describe('DrinkRecipeManager', () => {
  it('renders a list of recipes, showing a filled star for favorites', async () => {
    const oldFashioned = makeDrink({ id: 'd1', name: 'Old Fashioned', is_favorite: true })
    const daiquiri = makeDrink({ id: 'd2', name: 'Daiquiri', is_favorite: false })
    mockJson('GET', '/api/drink-recipes', [oldFashioned, daiquiri])

    renderWithProviders(<DrinkRecipeManager />)

    await waitFor(() => expect(screen.getByText('Old Fashioned')).toBeInTheDocument())
    expect(screen.getByText('Daiquiri')).toBeInTheDocument()

    // Favorite toggle reflects backend state: the favorited row has the
    // "Unfavorite" aria-label, the non-favorite has "Favorite".
    expect(screen.getByRole('button', { name: 'Unfavorite' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Favorite' })).toBeInTheDocument()
  })

  it('toggling favorite POSTs and invalidates the list (contract + behavior)', async () => {
    const daiquiri = makeDrink({ id: 'd2', name: 'Daiquiri', is_favorite: false })
    mockJson('GET', '/api/drink-recipes', [daiquiri])

    const { client } = renderWithProviders(<DrinkRecipeManager />)
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    await waitFor(() => expect(screen.getByText('Daiquiri')).toBeInTheDocument())

    // Stage: POST returns the favorited row; next GET reflects it.
    const favorited = { ...daiquiri, is_favorite: true }
    mockJson('POST', '/api/drink-recipes/d2/favorite', favorited)
    mockJson('GET', '/api/drink-recipes', [favorited])

    fireEvent.click(screen.getByRole('button', { name: 'Favorite' }))

    // Behavior: the list refetches and the star flips.
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Unfavorite' })).toBeInTheDocument()
    })
    // Contract: invalidation uses the canonical query key.
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['drink-recipes'] })
  })

  it('deleting a recipe (via the delete hook on any surface) removes it from the list on refetch', async () => {
    // The list view itself has no inline delete — delete lives on the detail
    // page. This test proves the invalidation contract: ANY caller of
    // useDeleteDrinkRecipe causes the list to refetch and drop the row.
    const oldFashioned = makeDrink({ id: 'd1', name: 'Old Fashioned' })
    const daiquiri = makeDrink({ id: 'd2', name: 'Daiquiri' })
    mockJson('GET', '/api/drink-recipes', [oldFashioned, daiquiri])

    const { client } = renderWithProviders(<DrinkRecipeManager />)
    await waitFor(() => expect(screen.getByText('Old Fashioned')).toBeInTheDocument())

    // Share the QueryClient so the mutation and the manager see the same cache.
    const Wrapper = ({ children }: { children: React.ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    )
    mockJson('DELETE', '/api/drink-recipes/d1', null, { status: 204 })
    mockJson('GET', '/api/drink-recipes', [daiquiri]) // post-delete refetch

    const { result } = renderHook(() => useDeleteDrinkRecipe(), { wrapper: Wrapper })
    result.current.mutate('d1')

    await waitFor(() => expect(screen.queryByText('Old Fashioned')).not.toBeInTheDocument())
    expect(screen.getByText('Daiquiri')).toBeInTheDocument()
  })

  it('search input filters the visible recipes without hitting the network', async () => {
    // Need > 3 recipes for the search input to render (component gate).
    const recipes = [
      makeDrink({ id: 'd1', name: 'Old Fashioned' }),
      makeDrink({ id: 'd2', name: 'Daiquiri' }),
      makeDrink({ id: 'd3', name: 'Negroni' }),
      makeDrink({ id: 'd4', name: 'Mojito' }),
    ]
    mockJson('GET', '/api/drink-recipes', recipes)

    renderWithProviders(<DrinkRecipeManager />)
    await waitFor(() => expect(screen.getByText('Old Fashioned')).toBeInTheDocument())
    const callsBefore = vi.mocked(fetch).mock.calls.length

    fireEvent.change(screen.getByPlaceholderText('Search drinks...'), {
      target: { value: 'negr' },
    })

    expect(screen.getByText('Negroni')).toBeInTheDocument()
    expect(screen.queryByText('Daiquiri')).not.toBeInTheDocument()
    expect(screen.queryByText('Mojito')).not.toBeInTheDocument()
    // No network request triggered by filtering.
    expect(vi.mocked(fetch).mock.calls.length).toBe(callsBefore)
  })
})
