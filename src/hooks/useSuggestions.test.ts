import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { createQueryWrapper } from '../test/queryClient'
import { installStreamMock, mockStream, resetStreamMock } from '../test/streamMock'
import type { CreateRecipeDto } from '../types/recipe'
import type { MealSuggestions } from '../types/suggestion'
import { useAiSuggestMeals, useMealSuggestions } from './useSuggestions'

beforeEach(() => {
  installFetchMock()
  installStreamMock()
})
afterEach(() => {
  resetFetchMock()
  resetStreamMock()
})

describe('useMealSuggestions', () => {
  it('POSTs to /api/suggestions with the given DTO and returns MealSuggestions', async () => {
    const response: MealSuggestions = {
      recent_favorites: [],
      forgotten_hits: [],
      untried: [
        {
          recipe_id: 'r1',
          recipe_name: 'Pasta',
          rating: 4,
          last_made: null,
          times_made: 0,
          reason: 'Never tried',
        },
      ],
    }
    mockJson('POST', '/api/suggestions', response)
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useMealSuggestions(), { wrapper: Wrapper })

    act(() => {
      result.current.mutate({
        person_ids: ['p1', 'p2'],
        reference_date: '2026-04-20',
      })
    })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(response)
    expect(fetch).toHaveBeenCalledWith(
      '/api/suggestions',
      expect.objectContaining({ method: 'POST' }),
    )
  })

  it('does not invalidate any queries (suggestions is read-only)', async () => {
    mockJson('POST', '/api/suggestions', {
      recent_favorites: [],
      forgotten_hits: [],
      untried: [],
    })
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useMealSuggestions(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({
        person_ids: ['p1'],
        reference_date: '2026-04-20',
      })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(invalidateSpy).not.toHaveBeenCalled()
  })
})

describe('useAiSuggestMeals', () => {
  it('streams from /suggestions/ai — sends input body, emits progress, then completes', async () => {
    const stream = mockStream<CreateRecipeDto[]>('/suggestions/ai')
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useAiSuggestMeals(), { wrapper: Wrapper })

    act(() => {
      result.current.mutate({
        person_options: [
          {
            person_id: 'p1',
            include_dietary_goals: true,
            include_dislikes: true,
            include_favorites: false,
          },
        ],
        meal_type: 'Dinner',
        character: { type: 'balanced' },
      })
    })

    expect(result.current.isPending).toBe(true)
    expect(stream.calls).toHaveLength(1)
    const body = stream.calls[0].body as {
      person_options: unknown[]
      meal_type: string
      character: { type: string }
    }
    expect(body.meal_type).toBe('Dinner')
    expect(body.character).toEqual({ type: 'balanced' })

    act(() => {
      stream.emit({ phase: 'thinking', message: 'Considering preferences…' })
    })
    expect(result.current.progress?.message).toBe('Considering preferences…')

    const suggestions: CreateRecipeDto[] = [
      {
        name: 'Suggested Pasta',
        source: 'ai',
        servings: 4,
        instructions: 'Cook pasta',
        ingredients: [],
        tags: ['ai-suggested'],
      },
    ]
    act(() => {
      stream.complete(suggestions)
    })

    expect(result.current.data).toEqual(suggestions)
    expect(result.current.isPending).toBe(false)
    expect(result.current.progress).toBeNull()
  })

  it('surfaces stream errors', async () => {
    const stream = mockStream<CreateRecipeDto[]>('/suggestions/ai')
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useAiSuggestMeals(), { wrapper: Wrapper })

    act(() => {
      result.current.mutate({
        person_options: [],
        meal_type: 'Dinner',
        character: { type: 'balanced' },
      })
    })
    act(() => {
      stream.error(new Error('rate limited'))
    })

    expect(result.current.error?.message).toBe('rate limited')
    expect(result.current.isPending).toBe(false)
  })
})
