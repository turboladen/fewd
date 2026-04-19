import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError } from '../lib/api'
import { makeMealTemplate } from '../test/factories'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { createQueryWrapper } from '../test/queryClient'
import {
  useCreateMealTemplate,
  useCreateTemplateFromMeal,
  useDeleteMealTemplate,
  useMealTemplates,
  useUpdateMealTemplate,
} from './useMealTemplates'

beforeEach(() => installFetchMock())
afterEach(() => resetFetchMock())

describe('useMealTemplates', () => {
  it('starts in loading state before the request resolves', () => {
    mockJson('GET', '/api/meal-templates', [makeMealTemplate()])
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useMealTemplates(), { wrapper: Wrapper })

    expect(result.current.isLoading).toBe(true)
  })

  it('returns the list on success', async () => {
    const dinner = makeMealTemplate({ id: 't1', name: 'Family Dinner' })
    const lunch = makeMealTemplate({ id: 't2', name: 'Lunch Box', meal_type: 'Lunch' })
    mockJson('GET', '/api/meal-templates', [dinner, lunch])
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useMealTemplates(), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual([dinner, lunch])
  })

  it('surfaces ApiError on non-2xx responses', async () => {
    mockJson('GET', '/api/meal-templates', { message: 'boom' }, { status: 500 })
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useMealTemplates(), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isError).toBe(true))
    expect((result.current.error as ApiError).status).toBe(500)
  })
})

describe('useCreateMealTemplate', () => {
  it('POSTs to /api/meal-templates and returns the created template', async () => {
    const created = makeMealTemplate({ id: 'new-1', name: 'Quick Breakfast' })
    mockJson('POST', '/api/meal-templates', created)
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useCreateMealTemplate(), { wrapper: Wrapper })

    act(() => {
      result.current.mutate({ name: 'Quick Breakfast', meal_type: 'Breakfast', servings: [] })
    })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(created)
    expect(fetch).toHaveBeenCalledWith(
      '/api/meal-templates',
      expect.objectContaining({ method: 'POST' }),
    )
  })

  it('invalidates the meal_templates list — sibling useMealTemplates refetches', async () => {
    mockJson('GET', '/api/meal-templates', [])
    mockJson('POST', '/api/meal-templates', makeMealTemplate({ id: 'new-1' }))
    const { Wrapper } = createQueryWrapper()

    const list = renderHook(() => useMealTemplates(), { wrapper: Wrapper })
    await waitFor(() => expect(list.result.current.isSuccess).toBe(true))

    const create = renderHook(() => useCreateMealTemplate(), { wrapper: Wrapper })
    act(() => {
      create.result.current.mutate({ name: 'T', meal_type: 'Dinner', servings: [] })
    })
    await waitFor(() => expect(create.result.current.isSuccess).toBe(true))

    await waitFor(() => {
      const getCalls = vi.mocked(fetch).mock.calls.filter(([, init]) => !init)
      expect(getCalls).toHaveLength(2)
    })
  })

  it('invalidates with the ["meal_templates"] query key (contract assertion)', async () => {
    mockJson('POST', '/api/meal-templates', makeMealTemplate({ id: 'new-1' }))
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useCreateMealTemplate(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({ name: 'T', meal_type: 'Dinner', servings: [] })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['meal_templates'] })
  })
})

describe('useUpdateMealTemplate', () => {
  it('PUTs to /api/meal-templates/:id and invalidates the list', async () => {
    const updated = makeMealTemplate({ id: 't1', name: 'Renamed' })
    mockJson('PUT', '/api/meal-templates/t1', updated)
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useUpdateMealTemplate(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({ id: 't1', data: { name: 'Renamed' } })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(result.current.data).toEqual(updated)
    expect(fetch).toHaveBeenCalledWith(
      '/api/meal-templates/t1',
      expect.objectContaining({ method: 'PUT' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['meal_templates'] })
  })
})

describe('useDeleteMealTemplate', () => {
  it('sends DELETE to /api/meal-templates/:id and invalidates the list', async () => {
    mockJson('DELETE', '/api/meal-templates/t1', null, { status: 204 })
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useDeleteMealTemplate(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate('t1')
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(fetch).toHaveBeenCalledWith(
      '/api/meal-templates/t1',
      expect.objectContaining({ method: 'DELETE' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['meal_templates'] })
  })
})

describe('useCreateTemplateFromMeal', () => {
  it('POSTs to /api/meal-templates/from-meal and invalidates the list', async () => {
    const created = makeMealTemplate({ id: 'new-1', name: 'Saved from Monday dinner' })
    mockJson('POST', '/api/meal-templates/from-meal', created)
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useCreateTemplateFromMeal(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({ meal_id: 'm1', name: 'Saved from Monday dinner' })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(result.current.data).toEqual(created)
    expect(fetch).toHaveBeenCalledWith(
      '/api/meal-templates/from-meal',
      expect.objectContaining({ method: 'POST' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['meal_templates'] })
  })
})
