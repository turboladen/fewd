import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError } from '../lib/api'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { createQueryWrapper } from '../test/queryClient'
import { installStreamMock, mockStream, resetStreamMock } from '../test/streamMock'
import type { DrinkRecipe } from '../types/drinkRecipe'
import {
  useCreateDrinkRecipe,
  useDeleteDrinkRecipe,
  useDrinkRecipe,
  useDrinkRecipes,
  useImportDrinkRecipeFromUrl,
  useToggleDrinkFavorite,
  useUpdateDrinkRecipe,
} from './useDrinkRecipes'

function makeDrinkRecipe(overrides: Partial<DrinkRecipe> = {}): DrinkRecipe {
  return {
    id: 'dr1',
    slug: 'old-fashioned',
    name: 'Old Fashioned',
    description: null,
    source: 'manual',
    source_url: null,
    servings: 1,
    instructions: 'Stir with ice.',
    ingredients: JSON.stringify([]),
    technique: null,
    glassware: null,
    garnish: null,
    tags: JSON.stringify([]),
    notes: null,
    icon: null,
    is_favorite: false,
    is_non_alcoholic: false,
    rating: null,
    times_made: 0,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

beforeEach(() => {
  installFetchMock()
  installStreamMock()
})
afterEach(() => {
  resetFetchMock()
  resetStreamMock()
})

describe('useDrinkRecipes', () => {
  it('starts in loading state before the request resolves', () => {
    mockJson('GET', '/api/drink-recipes', [makeDrinkRecipe()])
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useDrinkRecipes(), { wrapper: Wrapper })

    expect(result.current.isLoading).toBe(true)
  })

  it('returns the list on success', async () => {
    const oldFashioned = makeDrinkRecipe({ id: 'dr1', name: 'Old Fashioned' })
    const negroni = makeDrinkRecipe({ id: 'dr2', name: 'Negroni' })
    mockJson('GET', '/api/drink-recipes', [oldFashioned, negroni])
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useDrinkRecipes(), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual([oldFashioned, negroni])
  })

  it('surfaces ApiError on non-2xx responses', async () => {
    mockJson('GET', '/api/drink-recipes', { message: 'boom' }, { status: 500 })
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useDrinkRecipes(), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isError).toBe(true))
    expect((result.current.error as ApiError).status).toBe(500)
  })
})

describe('useDrinkRecipe', () => {
  it('does not fetch when id is empty', () => {
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useDrinkRecipe(''), { wrapper: Wrapper })

    expect(result.current.fetchStatus).toBe('idle')
    expect(fetch).not.toHaveBeenCalled()
  })

  it('fetches a single drink recipe when id is present', async () => {
    const drink = makeDrinkRecipe({ id: 'dr1', name: 'Old Fashioned' })
    mockJson('GET', '/api/drink-recipes/dr1', drink)
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useDrinkRecipe('dr1'), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(drink)
  })
})

describe('useCreateDrinkRecipe', () => {
  it('POSTs to /api/drink-recipes and returns the created drink', async () => {
    const created = makeDrinkRecipe({ id: 'new-1', name: 'Martini' })
    mockJson('POST', '/api/drink-recipes', created)
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useCreateDrinkRecipe(), { wrapper: Wrapper })

    act(() => {
      result.current.mutate({
        name: 'Martini',
        source: 'manual',
        servings: 1,
        instructions: 'Stir.',
        ingredients: [],
        tags: [],
      })
    })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(created)
    expect(fetch).toHaveBeenCalledWith(
      '/api/drink-recipes',
      expect.objectContaining({ method: 'POST' }),
    )
  })

  it('invalidates the drink-recipes list — sibling useDrinkRecipes refetches', async () => {
    mockJson('GET', '/api/drink-recipes', [])
    mockJson('POST', '/api/drink-recipes', makeDrinkRecipe({ id: 'new-1' }))
    const { Wrapper } = createQueryWrapper()

    const list = renderHook(() => useDrinkRecipes(), { wrapper: Wrapper })
    await waitFor(() => expect(list.result.current.isSuccess).toBe(true))

    const create = renderHook(() => useCreateDrinkRecipe(), { wrapper: Wrapper })
    act(() => {
      create.result.current.mutate({
        name: 'Martini',
        source: 'manual',
        servings: 1,
        instructions: 'Stir.',
        ingredients: [],
        tags: [],
      })
    })
    await waitFor(() => expect(create.result.current.isSuccess).toBe(true))

    await waitFor(() => {
      const getCalls = vi.mocked(fetch).mock.calls.filter(([, init]) => !init)
      expect(getCalls).toHaveLength(2)
    })
  })

  it('invalidates with the ["drink-recipes"] query key (contract assertion)', async () => {
    mockJson('POST', '/api/drink-recipes', makeDrinkRecipe({ id: 'new-1' }))
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useCreateDrinkRecipe(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({
        name: 'Martini',
        source: 'manual',
        servings: 1,
        instructions: 'Stir.',
        ingredients: [],
        tags: [],
      })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['drink-recipes'] })
  })
})

describe('useUpdateDrinkRecipe', () => {
  it('PUTs to /api/drink-recipes/:id and invalidates the list', async () => {
    const updated = makeDrinkRecipe({ id: 'dr1', name: 'Renamed' })
    mockJson('PUT', '/api/drink-recipes/dr1', updated)
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useUpdateDrinkRecipe(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({ id: 'dr1', data: { name: 'Renamed' } })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(result.current.data).toEqual(updated)
    expect(fetch).toHaveBeenCalledWith(
      '/api/drink-recipes/dr1',
      expect.objectContaining({ method: 'PUT' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['drink-recipes'] })
  })
})

describe('useDeleteDrinkRecipe', () => {
  it('sends DELETE to /api/drink-recipes/:id and invalidates the list', async () => {
    mockJson('DELETE', '/api/drink-recipes/dr1', null, { status: 204 })
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useDeleteDrinkRecipe(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate('dr1')
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(fetch).toHaveBeenCalledWith(
      '/api/drink-recipes/dr1',
      expect.objectContaining({ method: 'DELETE' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['drink-recipes'] })
  })
})

describe('useToggleDrinkFavorite', () => {
  it('POSTs to /api/drink-recipes/:id/favorite and invalidates the list', async () => {
    const updated = makeDrinkRecipe({ id: 'dr1', is_favorite: true })
    mockJson('POST', '/api/drink-recipes/dr1/favorite', updated)
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useToggleDrinkFavorite(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate('dr1')
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(result.current.data).toEqual(updated)
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['drink-recipes'] })
  })
})

describe('useImportDrinkRecipeFromUrl', () => {
  it('streams from /drink-recipes/import/url — progress, then complete', async () => {
    const stream = mockStream<DrinkRecipe>('/drink-recipes/import/url')
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useImportDrinkRecipeFromUrl(), { wrapper: Wrapper })

    act(() => {
      result.current.mutate({ url: 'https://example.com/recipe' })
    })
    expect(result.current.isPending).toBe(true)
    expect(stream.calls).toHaveLength(1)
    expect(stream.calls[0].body).toEqual({ url: 'https://example.com/recipe' })

    act(() => {
      stream.emit({ phase: 'thinking', message: 'Fetching page…' })
    })
    expect(result.current.progress?.message).toBe('Fetching page…')

    const imported = makeDrinkRecipe({ id: 'dr1', name: 'Imported Cocktail' })
    act(() => {
      stream.complete(imported)
    })

    expect(result.current.data).toEqual(imported)
    expect(result.current.isPending).toBe(false)
    expect(result.current.progress).toBeNull()
  })

  it('surfaces stream errors', async () => {
    const stream = mockStream<DrinkRecipe>('/drink-recipes/import/url')
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useImportDrinkRecipeFromUrl(), { wrapper: Wrapper })

    act(() => {
      result.current.mutate({ url: 'https://example.com/bad' })
    })
    act(() => {
      stream.error(new Error('URL not reachable'))
    })

    expect(result.current.error?.message).toBe('URL not reachable')
    expect(result.current.isPending).toBe(false)
  })
})
