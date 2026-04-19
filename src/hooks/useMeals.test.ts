import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError } from '../lib/api'
import { makeMeal } from '../test/factories'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { createQueryWrapper } from '../test/queryClient'
import {
  useCreateMeal,
  useDeleteMeal,
  useMeal,
  useMealsForDateRange,
  useUpdateMeal,
} from './useMeals'

beforeEach(() => installFetchMock())
afterEach(() => resetFetchMock())

describe('useMealsForDateRange', () => {
  it('does not fetch when startDate is empty', () => {
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useMealsForDateRange('', '2026-04-30'), {
      wrapper: Wrapper,
    })

    expect(result.current.fetchStatus).toBe('idle')
    expect(fetch).not.toHaveBeenCalled()
  })

  it('does not fetch when endDate is empty', () => {
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useMealsForDateRange('2026-04-20', ''), {
      wrapper: Wrapper,
    })

    expect(result.current.fetchStatus).toBe('idle')
    expect(fetch).not.toHaveBeenCalled()
  })

  it('fetches the list for a date range', async () => {
    const monday = makeMeal({ id: 'm1', date: '2026-04-20' })
    const tuesday = makeMeal({ id: 'm2', date: '2026-04-21' })
    mockJson(
      'GET',
      '/api/meals?start_date=2026-04-20&end_date=2026-04-30',
      [monday, tuesday],
    )
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useMealsForDateRange('2026-04-20', '2026-04-30'), {
      wrapper: Wrapper,
    })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual([monday, tuesday])
  })

  it('surfaces ApiError on non-2xx responses', async () => {
    mockJson(
      'GET',
      '/api/meals?start_date=2026-04-20&end_date=2026-04-30',
      { message: 'boom' },
      { status: 500 },
    )
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useMealsForDateRange('2026-04-20', '2026-04-30'), {
      wrapper: Wrapper,
    })

    await waitFor(() => expect(result.current.isError).toBe(true))
    expect((result.current.error as ApiError).status).toBe(500)
  })
})

describe('useMeal', () => {
  it('does not fetch when id is empty', () => {
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useMeal(''), { wrapper: Wrapper })

    expect(result.current.fetchStatus).toBe('idle')
    expect(fetch).not.toHaveBeenCalled()
  })

  it('fetches a single meal when id is present', async () => {
    const meal = makeMeal({ id: 'm1' })
    mockJson('GET', '/api/meals/m1', meal)
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useMeal('m1'), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(meal)
  })
})

describe('useCreateMeal', () => {
  it('POSTs to /api/meals and returns the created meal', async () => {
    const created = makeMeal({ id: 'new-1' })
    mockJson('POST', '/api/meals', created)
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useCreateMeal(), { wrapper: Wrapper })

    act(() => {
      result.current.mutate({
        date: '2026-04-20',
        meal_type: 'Dinner',
        order_index: 2,
        servings: [],
      })
    })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(created)
    expect(fetch).toHaveBeenCalledWith(
      '/api/meals',
      expect.objectContaining({ method: 'POST' }),
    )
  })

  it('invalidates both ["meals"] and ["recipes"] query keys', async () => {
    mockJson('POST', '/api/meals', makeMeal({ id: 'new-1' }))
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useCreateMeal(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({
        date: '2026-04-20',
        meal_type: 'Dinner',
        order_index: 2,
        servings: [],
      })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['meals'] })
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['recipes'] })
  })

  it('behaviorally refetches sibling date-range query after create', async () => {
    mockJson('GET', '/api/meals?start_date=2026-04-20&end_date=2026-04-30', [])
    mockJson('POST', '/api/meals', makeMeal({ id: 'new-1' }))
    const { Wrapper } = createQueryWrapper()

    const list = renderHook(() => useMealsForDateRange('2026-04-20', '2026-04-30'), {
      wrapper: Wrapper,
    })
    await waitFor(() => expect(list.result.current.isSuccess).toBe(true))

    const create = renderHook(() => useCreateMeal(), { wrapper: Wrapper })
    act(() => {
      create.result.current.mutate({
        date: '2026-04-20',
        meal_type: 'Dinner',
        order_index: 2,
        servings: [],
      })
    })
    await waitFor(() => expect(create.result.current.isSuccess).toBe(true))

    await waitFor(() => {
      const getCalls = vi.mocked(fetch).mock.calls.filter(([, init]) => !init)
      expect(getCalls).toHaveLength(2)
    })
  })
})

describe('useUpdateMeal', () => {
  it('PUTs to /api/meals/:id and invalidates both meals and recipes', async () => {
    const updated = makeMeal({ id: 'm1', meal_type: 'Lunch' })
    mockJson('PUT', '/api/meals/m1', updated)
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useUpdateMeal(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({ id: 'm1', data: { meal_type: 'Lunch' } })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(result.current.data).toEqual(updated)
    expect(fetch).toHaveBeenCalledWith(
      '/api/meals/m1',
      expect.objectContaining({ method: 'PUT' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['meals'] })
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['recipes'] })
  })
})

describe('useDeleteMeal', () => {
  it('sends DELETE to /api/meals/:id and invalidates only meals (not recipes)', async () => {
    mockJson('DELETE', '/api/meals/m1', null, { status: 204 })
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useDeleteMeal(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate('m1')
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(fetch).toHaveBeenCalledWith(
      '/api/meals/m1',
      expect.objectContaining({ method: 'DELETE' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['meals'] })
    expect(invalidateSpy).not.toHaveBeenCalledWith({ queryKey: ['recipes'] })
  })
})
