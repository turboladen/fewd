import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError } from '../lib/api'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { createQueryWrapper } from '../test/queryClient'
import type { ModelOption, TokenUsage } from '../types/settings'
import {
  useAvailableModels,
  useSetSetting,
  useSetting,
  useTestConnection,
  useTokenUsage,
} from './useSettings'

beforeEach(() => installFetchMock())
afterEach(() => resetFetchMock())

describe('useSetting', () => {
  it('fetches GET /api/settings/:key with the key in the query key', async () => {
    mockJson('GET', '/api/settings/theme', 'dark')
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useSetting('theme'), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toBe('dark')
  })

  it('returns null value on success when backend returns null', async () => {
    mockJson('GET', '/api/settings/theme', null)
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useSetting('theme'), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toBeNull()
  })

  it('surfaces ApiError on non-2xx responses', async () => {
    mockJson('GET', '/api/settings/theme', { message: 'boom' }, { status: 500 })
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useSetting('theme'), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isError).toBe(true))
    expect((result.current.error as ApiError).status).toBe(500)
  })
})

describe('useSetSetting', () => {
  it('PUTs to /api/settings/:key and invalidates only ["settings", key] for non-API-key settings', async () => {
    mockJson('PUT', '/api/settings/theme', null)
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useSetSetting(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({ key: 'theme', value: 'dark' })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(fetch).toHaveBeenCalledWith(
      '/api/settings/theme',
      expect.objectContaining({ method: 'PUT' }),
    )
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['settings', 'theme'] })
    expect(invalidateSpy).not.toHaveBeenCalledWith({ queryKey: ['available-models'] })
  })

  it('also invalidates ["available-models"] when the key is anthropic_api_key', async () => {
    mockJson('PUT', '/api/settings/anthropic_api_key', null)
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useSetSetting(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate({ key: 'anthropic_api_key', value: 'sk-ant-xxx' })
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ['settings', 'anthropic_api_key'],
    })
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['available-models'] })
  })

  it('behaviorally refetches the specific setting after update', async () => {
    mockJson('GET', '/api/settings/theme', 'light')
    mockJson('PUT', '/api/settings/theme', null)
    const { Wrapper } = createQueryWrapper()

    const read = renderHook(() => useSetting('theme'), { wrapper: Wrapper })
    await waitFor(() => expect(read.result.current.isSuccess).toBe(true))

    const set = renderHook(() => useSetSetting(), { wrapper: Wrapper })
    act(() => {
      set.result.current.mutate({ key: 'theme', value: 'dark' })
    })
    await waitFor(() => expect(set.result.current.isSuccess).toBe(true))

    await waitFor(() => {
      const getCalls = vi.mocked(fetch).mock.calls.filter(([, init]) => !init)
      expect(getCalls).toHaveLength(2)
    })
  })
})

describe('useAvailableModels', () => {
  it('fetches GET /api/settings/models', async () => {
    const models: ModelOption[] = [
      { id: 'claude-sonnet-4-20250514', name: 'Claude Sonnet 4' },
    ]
    mockJson('GET', '/api/settings/models', models)
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useAvailableModels(), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(models)
  })
})

describe('useTestConnection', () => {
  it('POSTs to /api/settings/test-connection and does not invalidate anything', async () => {
    mockJson('POST', '/api/settings/test-connection', 'Connection OK')
    const { Wrapper, client } = createQueryWrapper()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    const { result } = renderHook(() => useTestConnection(), { wrapper: Wrapper })
    act(() => {
      result.current.mutate()
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(result.current.data).toBe('Connection OK')
    expect(fetch).toHaveBeenCalledWith(
      '/api/settings/test-connection',
      expect.objectContaining({ method: 'POST' }),
    )
    expect(invalidateSpy).not.toHaveBeenCalled()
  })
})

describe('useTokenUsage', () => {
  it('fetches GET /api/settings/token-usage', async () => {
    const usage: TokenUsage = {
      input_tokens: 1234,
      output_tokens: 567,
      total_requests: 42,
    }
    mockJson('GET', '/api/settings/token-usage', usage)
    const { Wrapper } = createQueryWrapper()

    const { result } = renderHook(() => useTokenUsage(), { wrapper: Wrapper })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(usage)
  })
})
