import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { createQueryWrapper } from '../test/queryClient'
import { installStreamMock, mockStream, resetStreamMock } from '../test/streamMock'
import type { AiSuggestCocktailsDto, CreateDrinkRecipeDto } from '../types/drinkRecipe'
import { useAiSuggestCocktails } from './useCocktailSuggestions'

beforeEach(() => installStreamMock())
afterEach(() => resetStreamMock())

describe('useAiSuggestCocktails', () => {
  it('posts to /cocktails/suggest with the DTO body', () => {
    const stream = mockStream<CreateDrinkRecipeDto[]>('/cocktails/suggest')
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useAiSuggestCocktails(), { wrapper: Wrapper })

    const input: AiSuggestCocktailsDto = {
      person_ids: ['p1'],
      bar_item_ids: ['bi-1', 'bi-2'],
      mood: { type: 'style', label: 'Bright' },
      include_non_alcoholic: false,
    }
    act(() => {
      result.current.mutate(input)
    })

    expect(stream.calls).toHaveLength(1)
    expect(stream.calls[0].body).toEqual(input)
    expect(result.current.isPending).toBe(true)
  })

  it('updates progress state on emitted thinking events', () => {
    const stream = mockStream<CreateDrinkRecipeDto[]>('/cocktails/suggest')
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useAiSuggestCocktails(), { wrapper: Wrapper })

    act(() => {
      result.current.mutate({
        person_ids: [],
        bar_item_ids: [],
        mood: { type: 'custom', text: 'Summery' },
        include_non_alcoholic: true,
      })
    })
    act(() => {
      stream.emit({ phase: 'thinking', message: 'Brewing ideas…' })
    })

    expect(result.current.progress?.message).toBe('Brewing ideas…')
    expect(result.current.progress?.phase).toBe('thinking')
  })

  it('populates data, clears progress, and flips isPending on complete', () => {
    const stream = mockStream<CreateDrinkRecipeDto[]>('/cocktails/suggest')
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useAiSuggestCocktails(), { wrapper: Wrapper })

    const suggestion: CreateDrinkRecipeDto = {
      name: 'House Sazerac',
      source: 'ai',
      servings: 1,
      instructions: 'Stir with absinthe rinse.',
      ingredients: [],
      tags: ['ai-suggested'],
    }

    act(() => {
      result.current.mutate({
        person_ids: [],
        bar_item_ids: [],
        mood: { type: 'style', label: 'Classic' },
        include_non_alcoholic: false,
      })
    })
    act(() => {
      stream.emit({ phase: 'generating', message: 'Writing recipe…' })
    })
    act(() => {
      stream.complete([suggestion])
    })

    expect(result.current.data).toEqual([suggestion])
    expect(result.current.isPending).toBe(false)
    expect(result.current.progress).toBeNull()
    expect(result.current.error).toBeNull()
  })

  it('captures errors and clears isPending', () => {
    const stream = mockStream<CreateDrinkRecipeDto[]>('/cocktails/suggest')
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useAiSuggestCocktails(), { wrapper: Wrapper })

    act(() => {
      result.current.mutate({
        person_ids: [],
        bar_item_ids: [],
        mood: { type: 'style', label: 'Classic' },
        include_non_alcoholic: false,
      })
    })
    act(() => {
      stream.error(new Error('AI endpoint down'))
    })

    expect(result.current.error?.message).toBe('AI endpoint down')
    expect(result.current.isPending).toBe(false)
    expect(result.current.data).toBeUndefined()
  })
})
