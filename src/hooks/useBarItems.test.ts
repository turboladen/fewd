import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError } from '../lib/api'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { createQueryWrapper } from '../test/queryClient'
import type { BarItem } from '../types/barItem'
import {
  useBarItems,
  useBulkCreateBarItems,
  useClearBarItems,
  useCreateBarItem,
  useDeleteBarItem,
} from './useBarItems'

function makeBarItem(overrides: Partial<BarItem> = {}): BarItem {
  return {
    id: 'bi-1',
    name: 'Bourbon',
    category: 'spirit',
    created_at: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

beforeEach(() => installFetchMock())
afterEach(() => resetFetchMock())

describe('useBarItems', () => {
  it('starts in loading state before the request resolves', () => {
    mockJson('GET', '/api/bar-items', [makeBarItem()])
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useBarItems(), { wrapper: Wrapper })

    expect(result.current.isLoading).toBe(true)
  })

  it('returns the list on success', async () => {
    const bourbon = makeBarItem({ id: 'bi-1', name: 'Bourbon' })
    const lime = makeBarItem({ id: 'bi-2', name: 'Lime Juice', category: 'juice' })
    mockJson('GET', '/api/bar-items', [bourbon, lime])
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useBarItems(), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual([bourbon, lime])
  })

  it('surfaces ApiError on non-2xx responses', async () => {
    mockJson('GET', '/api/bar-items', { message: 'boom' }, { status: 500 })
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useBarItems(), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isError).toBe(true))
    expect((result.current.error as ApiError).status).toBe(500)
  })
})

describe('useCreateBarItem', () => {
  it('POSTs to /api/bar-items and returns the created item', async () => {
    const created = makeBarItem({ id: 'new-1', name: 'Rye' })
    mockJson('POST', '/api/bar-items', created)
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useCreateBarItem(), { wrapper: Wrapper })

    act(() => {
      result.current.mutate({ name: 'Rye', category: 'spirit' })
    })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(created)
    expect(fetch).toHaveBeenCalledWith(
      '/api/bar-items',
      expect.objectContaining({ method: 'POST' }),
    )
  })

  it('invalidates the bar-items list — sibling useBarItems refetches', async () => {
    mockJson('GET', '/api/bar-items', [])
    mockJson('POST', '/api/bar-items', makeBarItem({ id: 'new-1', name: 'Rye' }))
    const { Wrapper } = createQueryWrapper()

    const list = renderHook(() => useBarItems(), { wrapper: Wrapper })
    await waitFor(() => expect(list.result.current.isSuccess).toBe(true))

    const create = renderHook(() => useCreateBarItem(), { wrapper: Wrapper })
    act(() => {
      create.result.current.mutate({ name: 'Rye', category: 'spirit' })
    })
    await waitFor(() => expect(create.result.current.isSuccess).toBe(true))

    await waitFor(() => {
      const getCalls = vi.mocked(fetch).mock.calls.filter(([, init]) => !init)
      expect(getCalls).toHaveLength(2)
    })
  })

  it('invalidates with the ["bar-items"] query key (contract assertion)', async () => {
    mockJson('POST', '/api/bar-items', makeBarItem({ id: 'new-1' }))
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useCreateBarItem(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({ name: 'Rye', category: 'spirit' })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['bar-items'] })
  })
})

describe('useBulkCreateBarItems', () => {
  it('POSTs an array to /api/bar-items/bulk and invalidates the list', async () => {
    const items = [
      makeBarItem({ id: 'new-1', name: 'Rye' }),
      makeBarItem({ id: 'new-2', name: 'Gin' }),
    ]
    mockJson('POST', '/api/bar-items/bulk', items)
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useBulkCreateBarItems(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({
        items: [
          { name: 'Rye', category: 'spirit' },
          { name: 'Gin', category: 'spirit' },
        ],
      })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(result.current.data).toEqual(items)
    expect(fetch).toHaveBeenCalledWith(
      '/api/bar-items/bulk',
      expect.objectContaining({ method: 'POST' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['bar-items'] })
  })
})

describe('useDeleteBarItem', () => {
  it('sends DELETE to /api/bar-items/:id with a 204 response and invalidates the list', async () => {
    mockJson('DELETE', '/api/bar-items/bi-1', null, { status: 204 })
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useDeleteBarItem(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate('bi-1')
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(fetch).toHaveBeenCalledWith(
      '/api/bar-items/bi-1',
      expect.objectContaining({ method: 'DELETE' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['bar-items'] })
  })
})

describe('useClearBarItems', () => {
  it('sends DELETE to /api/bar-items/all and invalidates the list', async () => {
    mockJson('DELETE', '/api/bar-items/all', null, { status: 204 })
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useClearBarItems(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate()
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(fetch).toHaveBeenCalledWith(
      '/api/bar-items/all',
      expect.objectContaining({ method: 'DELETE' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['bar-items'] })
  })
})
