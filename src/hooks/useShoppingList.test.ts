import { renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { ApiError } from '../lib/api'
import { makeAggregatedIngredient } from '../test/factories'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { createQueryWrapper } from '../test/queryClient'
import { useShoppingList } from './useShoppingList'

beforeEach(() => installFetchMock())
afterEach(() => resetFetchMock())

describe('useShoppingList', () => {
  it('does not fetch when startDate is empty', () => {
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useShoppingList('', '2026-04-30'), { wrapper: Wrapper })

    expect(result.current.fetchStatus).toBe('idle')
    expect(fetch).not.toHaveBeenCalled()
  })

  it('does not fetch when endDate is empty', () => {
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useShoppingList('2026-04-20', ''), { wrapper: Wrapper })

    expect(result.current.fetchStatus).toBe('idle')
    expect(fetch).not.toHaveBeenCalled()
  })

  it('fetches the aggregated list when both dates are present', async () => {
    const tomatoes = makeAggregatedIngredient({ ingredient_name: 'Tomato' })
    const onions = makeAggregatedIngredient({ ingredient_name: 'Onion' })
    mockJson(
      'GET',
      '/api/shopping-list?start_date=2026-04-20&end_date=2026-04-30',
      [tomatoes, onions],
    )
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useShoppingList('2026-04-20', '2026-04-30'), {
      wrapper: Wrapper,
    })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual([tomatoes, onions])
  })

  it('surfaces ApiError on non-2xx responses', async () => {
    mockJson(
      'GET',
      '/api/shopping-list?start_date=2026-04-20&end_date=2026-04-30',
      { message: 'server down' },
      { status: 500 },
    )
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useShoppingList('2026-04-20', '2026-04-30'), {
      wrapper: Wrapper,
    })

    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(result.current.error).toBeInstanceOf(ApiError)
    expect((result.current.error as ApiError).status).toBe(500)
    expect((result.current.error as ApiError).message).toBe('server down')
  })
})
