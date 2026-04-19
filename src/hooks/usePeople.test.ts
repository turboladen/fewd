import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError } from '../lib/api'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { createQueryWrapper } from '../test/queryClient'
import type { Person } from '../types/person'
import {
  useCreatePerson,
  useDeletePerson,
  usePeople,
  usePerson,
  useUpdatePerson,
} from './usePeople'

function makePerson(overrides: Partial<Person> = {}): Person {
  return {
    id: 'p1',
    name: 'Alice',
    birthdate: '1990-01-01',
    dietary_goals: null,
    dislikes: '[]',
    favorites: '[]',
    notes: null,
    drink_preferences: null,
    drink_dislikes: null,
    is_active: true,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

beforeEach(() => installFetchMock())
afterEach(() => resetFetchMock())

describe('usePeople', () => {
  it('starts in loading state before the request resolves', () => {
    mockJson('GET', '/api/people', [makePerson()])
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => usePeople(), { wrapper: Wrapper })

    expect(result.current.isLoading).toBe(true)
    expect(result.current.data).toBeUndefined()
  })

  it('returns the list on success', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice' })
    const bob = makePerson({ id: 'p2', name: 'Bob' })
    mockJson('GET', '/api/people', [alice, bob])
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => usePeople(), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual([alice, bob])
  })

  it('surfaces ApiError on non-2xx responses', async () => {
    mockJson('GET', '/api/people', { message: 'boom' }, { status: 500 })
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => usePeople(), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(result.current.error).toBeInstanceOf(ApiError)
    expect((result.current.error as ApiError).status).toBe(500)
    expect((result.current.error as ApiError).message).toBe('boom')
  })
})

describe('usePerson', () => {
  it('does not fetch when id is empty', () => {
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => usePerson(''), { wrapper: Wrapper })

    expect(result.current.fetchStatus).toBe('idle')
    expect(fetch).not.toHaveBeenCalled()
  })

  it('fetches a single person when id is present', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice' })
    mockJson('GET', '/api/people/p1', alice)
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => usePerson('p1'), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(alice)
  })
})

describe('useCreatePerson', () => {
  it('POSTs to /api/people and returns the created person', async () => {
    const created = makePerson({ id: 'new-1', name: 'Charlie' })
    mockJson('POST', '/api/people', created)
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useCreatePerson(), { wrapper: Wrapper })

    act(() => {
      result.current.mutate({
        name: 'Charlie',
        birthdate: '1995-05-05',
        dislikes: [],
        favorites: [],
      })
    })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(created)
    expect(fetch).toHaveBeenCalledWith(
      '/api/people',
      expect.objectContaining({ method: 'POST' }),
    )
  })

  it('invalidates the people list — sibling usePeople refetches', async () => {
    mockJson('GET', '/api/people', [])
    mockJson('POST', '/api/people', makePerson({ id: 'new-1', name: 'Charlie' }))
    const { Wrapper } = createQueryWrapper()

    // Prime the cache with the initial list.
    const list = renderHook(() => usePeople(), { wrapper: Wrapper })
    await waitFor(() => expect(list.result.current.isSuccess).toBe(true))
    expect(vi.mocked(fetch).mock.calls).toHaveLength(1)

    // Mutate — should trigger invalidation → refetch of /api/people.
    const create = renderHook(() => useCreatePerson(), { wrapper: Wrapper })
    act(() => {
      create.result.current.mutate({
        name: 'Charlie',
        birthdate: '1995-05-05',
        dislikes: [],
        favorites: [],
      })
    })
    await waitFor(() => expect(create.result.current.isSuccess).toBe(true))

    // Two GETs (initial + refetch after invalidation) + one POST.
    await waitFor(() => {
      const getCalls = vi.mocked(fetch).mock.calls.filter(([, init]) => !init)
      expect(getCalls).toHaveLength(2)
    })
  })

  it('invalidates with the ["people"] query key (contract assertion)', async () => {
    mockJson('POST', '/api/people', makePerson({ id: 'new-1' }))
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useCreatePerson(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({
        name: 'Charlie',
        birthdate: '1995-05-05',
        dislikes: [],
        favorites: [],
      })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['people'] })
  })
})

describe('useUpdatePerson', () => {
  it('PUTs to /api/people/:id and invalidates the list', async () => {
    const updated = makePerson({ id: 'p1', name: 'Alice Renamed' })
    mockJson('PUT', '/api/people/p1', updated)
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useUpdatePerson(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({ id: 'p1', data: { name: 'Alice Renamed' } })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(result.current.data).toEqual(updated)
    expect(fetch).toHaveBeenCalledWith(
      '/api/people/p1',
      expect.objectContaining({ method: 'PUT' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['people'] })
  })
})

describe('useDeletePerson', () => {
  it('sends DELETE to /api/people/:id with a 204 response and invalidates the list', async () => {
    mockJson('DELETE', '/api/people/p1', null, { status: 204 })
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useDeletePerson(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate('p1')
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(fetch).toHaveBeenCalledWith(
      '/api/people/p1',
      expect.objectContaining({ method: 'DELETE' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['people'] })
  })
})
