import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError } from '../lib/api'
import { makeRecipe } from '../test/factories'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { createQueryWrapper } from '../test/queryClient'
import { installStreamMock, mockStream, resetStreamMock } from '../test/streamMock'
import type { CreateRecipeDto, Recipe, ScaleResult } from '../types/recipe'
import {
  useAdaptRecipe,
  useCreateRecipe,
  useDeleteRecipe,
  useEnhanceInstructions,
  useImportRecipe,
  useImportRecipeFromFile,
  useImportRecipeFromUrl,
  usePreviewScaleRecipe,
  useRecipe,
  useRecipes,
  useSearchRecipes,
  useToggleFavorite,
  useUpdateRecipe,
} from './useRecipes'

beforeEach(() => {
  installFetchMock()
  installStreamMock()
})
afterEach(() => {
  resetFetchMock()
  resetStreamMock()
})

describe('useRecipes', () => {
  it('starts in loading state before the request resolves', () => {
    mockJson('GET', '/api/recipes', [makeRecipe()])
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useRecipes(), { wrapper: Wrapper })

    expect(result.current.isLoading).toBe(true)
  })

  it('returns the list on success', async () => {
    const pasta = makeRecipe({ id: 'r1', name: 'Pasta' })
    const soup = makeRecipe({ id: 'r2', name: 'Soup' })
    mockJson('GET', '/api/recipes', [pasta, soup])
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useRecipes(), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual([pasta, soup])
  })

  it('surfaces ApiError on non-2xx responses', async () => {
    mockJson('GET', '/api/recipes', { message: 'boom' }, { status: 500 })
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useRecipes(), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isError).toBe(true))
    expect((result.current.error as ApiError).status).toBe(500)
  })
})

describe('useRecipe', () => {
  it('does not fetch when id is empty', () => {
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useRecipe(''), { wrapper: Wrapper })

    expect(result.current.fetchStatus).toBe('idle')
    expect(fetch).not.toHaveBeenCalled()
  })

  it('fetches a single recipe when id is present', async () => {
    const pasta = makeRecipe({ id: 'r1', name: 'Pasta' })
    mockJson('GET', '/api/recipes/r1', pasta)
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useRecipe('r1'), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(pasta)
  })
})

describe('useSearchRecipes', () => {
  it('does not fetch when query is empty', () => {
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useSearchRecipes(''), { wrapper: Wrapper })

    expect(result.current.fetchStatus).toBe('idle')
    expect(fetch).not.toHaveBeenCalled()
  })

  it('URL-encodes the query string', async () => {
    const pasta = makeRecipe({ id: 'r1', name: 'Pasta alla Vodka' })
    mockJson('GET', '/api/recipes/search?q=pasta%20vodka', [pasta])
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useSearchRecipes('pasta vodka'), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual([pasta])
  })
})

describe('useCreateRecipe', () => {
  const validDto: CreateRecipeDto = {
    name: 'New',
    source: 'manual',
    servings: 4,
    instructions: 'Cook.',
    ingredients: [],
    tags: [],
  }

  it('POSTs to /api/recipes and returns the created recipe', async () => {
    const created = makeRecipe({ id: 'new-1', name: 'New' })
    mockJson('POST', '/api/recipes', created)
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useCreateRecipe(), { wrapper: Wrapper })

    act(() => {
      result.current.mutate(validDto)
    })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(created)
    expect(fetch).toHaveBeenCalledWith(
      '/api/recipes',
      expect.objectContaining({ method: 'POST' }),
    )
  })

  it('invalidates the recipes list — sibling useRecipes refetches', async () => {
    mockJson('GET', '/api/recipes', [])
    mockJson('POST', '/api/recipes', makeRecipe({ id: 'new-1' }))
    const { Wrapper } = createQueryWrapper()

    const list = renderHook(() => useRecipes(), { wrapper: Wrapper })
    await waitFor(() => expect(list.result.current.isSuccess).toBe(true))

    const create = renderHook(() => useCreateRecipe(), { wrapper: Wrapper })
    act(() => {
      create.result.current.mutate(validDto)
    })
    await waitFor(() => expect(create.result.current.isSuccess).toBe(true))

    await waitFor(() => {
      const getCalls = vi.mocked(fetch).mock.calls.filter(([, init]) => !init)
      expect(getCalls).toHaveLength(2)
    })
  })

  it('invalidates with the ["recipes"] query key (contract assertion)', async () => {
    mockJson('POST', '/api/recipes', makeRecipe({ id: 'new-1' }))
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useCreateRecipe(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate(validDto)
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['recipes'] })
  })
})

describe('useUpdateRecipe', () => {
  it('PUTs to /api/recipes/:id and invalidates the list', async () => {
    const updated = makeRecipe({ id: 'r1', name: 'Renamed' })
    mockJson('PUT', '/api/recipes/r1', updated)
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useUpdateRecipe(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({ id: 'r1', data: { name: 'Renamed' } })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(result.current.data).toEqual(updated)
    expect(fetch).toHaveBeenCalledWith(
      '/api/recipes/r1',
      expect.objectContaining({ method: 'PUT' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['recipes'] })
  })
})

describe('useDeleteRecipe', () => {
  it('sends DELETE to /api/recipes/:id and invalidates the list', async () => {
    mockJson('DELETE', '/api/recipes/r1', null, { status: 204 })
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useDeleteRecipe(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate('r1')
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(fetch).toHaveBeenCalledWith(
      '/api/recipes/r1',
      expect.objectContaining({ method: 'DELETE' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['recipes'] })
  })
})

describe('useToggleFavorite', () => {
  it('POSTs to /api/recipes/:id/favorite and invalidates the list', async () => {
    const favorited = makeRecipe({ id: 'r1', is_favorite: true })
    mockJson('POST', '/api/recipes/r1/favorite', favorited)
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useToggleFavorite(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate('r1')
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(result.current.data).toEqual(favorited)
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['recipes'] })
  })
})

describe('usePreviewScaleRecipe', () => {
  it('POSTs to /api/recipes/:id/scale with { new_servings } and does NOT invalidate', async () => {
    const scaled: ScaleResult = { ingredients: [], flagged: [] }
    mockJson('POST', '/api/recipes/r1/scale', scaled)
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => usePreviewScaleRecipe(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({ id: 'r1', newServings: 2 })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(result.current.data).toEqual(scaled)
    expect(fetch).toHaveBeenCalledWith(
      '/api/recipes/r1/scale',
      expect.objectContaining({ method: 'POST' }),
    )
    expect(invalidateSpy).not.toHaveBeenCalled()
  })
})

describe('useEnhanceInstructions', () => {
  it('POSTs to /api/recipes/:id/enhance and returns a string with no invalidation', async () => {
    mockJson('POST', '/api/recipes/r1/enhance', 'Stir until fragrant.')
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useEnhanceInstructions(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate('r1')
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(result.current.data).toBe('Stir until fragrant.')
    expect(invalidateSpy).not.toHaveBeenCalled()
  })
})

describe('useAdaptRecipe', () => {
  it('streams from a dynamic path built from recipe_id', () => {
    const stream = mockStream<CreateRecipeDto>('/recipes/r1/adapt')
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useAdaptRecipe(), { wrapper: Wrapper })

    act(() => {
      result.current.mutate({
        recipe_id: 'r1',
        person_options: [
          {
            person_id: 'p1',
            include_dietary_goals: true,
            include_dislikes: false,
            include_favorites: false,
          },
        ],
        user_instructions: 'make it spicier',
      })
    })

    expect(stream.calls).toHaveLength(1)
    const body = stream.calls[0].body as { recipe_id: string; user_instructions: string }
    expect(body.recipe_id).toBe('r1')
    expect(body.user_instructions).toBe('make it spicier')

    const adapted: CreateRecipeDto = {
      name: 'Spicy Pasta',
      source: 'ai-adapted',
      servings: 4,
      instructions: 'Add chili.',
      ingredients: [],
      tags: [],
    }
    act(() => {
      stream.complete(adapted)
    })

    expect(result.current.data).toEqual(adapted)
  })
})

describe('useImportRecipe', () => {
  it('POSTs to /api/recipes/import/markdown and invalidates the list', async () => {
    const imported = makeRecipe({ id: 'new-1', name: 'Imported' })
    mockJson('POST', '/api/recipes/import/markdown', imported)
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useImportRecipe(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({ markdown: '# Imported\n\nCook.' })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(result.current.data).toEqual(imported)
    expect(fetch).toHaveBeenCalledWith(
      '/api/recipes/import/markdown',
      expect.objectContaining({ method: 'POST' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['recipes'] })
  })
})

describe('useImportRecipeFromUrl', () => {
  it('streams from /recipes/import/url and returns a Recipe on complete', () => {
    const stream = mockStream<Recipe>('/recipes/import/url')
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useImportRecipeFromUrl(), { wrapper: Wrapper })

    act(() => {
      result.current.mutate({ url: 'https://example.com/recipe' })
    })
    expect(stream.calls[0].body).toEqual({ url: 'https://example.com/recipe' })

    const imported = makeRecipe({ id: 'new-1', name: 'From URL' })
    act(() => {
      stream.complete(imported)
    })

    expect(result.current.data).toEqual(imported)
  })
})

describe('useImportRecipeFromFile', () => {
  it('POSTs a FormData body to /api/recipes/import/file and invalidates the list', async () => {
    const imported = makeRecipe({ id: 'new-1', name: 'From File' })
    mockJson('POST', '/api/recipes/import/file', imported)
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useImportRecipeFromFile(), { wrapper: Wrapper })

    const file = new File(['# Recipe'], 'recipe.md', { type: 'text/markdown' })
    act(() => {
      result.current.mutate(file)
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(result.current.data).toEqual(imported)

    const uploadCall = vi.mocked(fetch).mock.calls.find(
      ([url, init]) => url === '/api/recipes/import/file' && init?.method === 'POST',
    )
    expect(uploadCall).toBeDefined()
    expect(uploadCall?.[1]?.body).toBeInstanceOf(FormData)
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['recipes'] })
  })
})
