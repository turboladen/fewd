import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createQueryWrapper } from '../test/queryClient'
import { installStreamMock, mockStream, resetStreamMock } from '../test/streamMock'
import { useStreamingMutation } from './useStreamingMutation'

beforeEach(() => installStreamMock())
afterEach(() => resetStreamMock())

describe('useStreamingMutation — initial state', () => {
  it('starts idle: data undefined, error null, isPending false, progress null', () => {
    const { Wrapper } = createQueryWrapper()
    const { result } = renderHook(
      () => useStreamingMutation<{ id: string }, string>({ path: '/static' }),
      { wrapper: Wrapper },
    )

    expect(result.current.data).toBeUndefined()
    expect(result.current.error).toBeNull()
    expect(result.current.isPending).toBe(false)
    expect(result.current.progress).toBeNull()
  })
})

describe('useStreamingMutation — happy path', () => {
  it('transitions isPending → progress → data on complete()', () => {
    const stream = mockStream<string>('/static')
    const { Wrapper } = createQueryWrapper()
    const { result } = renderHook(
      () => useStreamingMutation<{ id: string }, string>({ path: '/static' }),
      { wrapper: Wrapper },
    )

    act(() => {
      result.current.mutate({ id: 'x' })
    })
    expect(result.current.isPending).toBe(true)

    act(() => {
      stream.emit({ phase: 'thinking', message: 'working…' })
    })
    expect(result.current.progress?.message).toBe('working…')

    act(() => {
      stream.complete('done')
    })
    expect(result.current.data).toBe('done')
    expect(result.current.isPending).toBe(false)
    expect(result.current.progress).toBeNull()
    expect(result.current.error).toBeNull()
  })
})

describe('useStreamingMutation — error path', () => {
  it('sets error and clears isPending on error()', () => {
    const stream = mockStream<string>('/static')
    const { Wrapper } = createQueryWrapper()
    const { result } = renderHook(
      () => useStreamingMutation<{ id: string }, string>({ path: '/static' }),
      { wrapper: Wrapper },
    )

    act(() => {
      result.current.mutate({ id: 'x' })
    })
    act(() => {
      stream.error(new Error('failed'))
    })

    expect(result.current.error?.message).toBe('failed')
    expect(result.current.isPending).toBe(false)
    expect(result.current.progress).toBeNull()
    expect(result.current.data).toBeUndefined()
  })
})

describe('useStreamingMutation — dynamic path', () => {
  it('resolves the path from the input when path is a function', () => {
    const stream = mockStream<string>('/things/abc/adapt')
    const { Wrapper } = createQueryWrapper()
    const { result } = renderHook(
      () =>
        useStreamingMutation<{ id: string }, string>({
          path: (input) => '/things/' + input.id + '/adapt',
        }),
      { wrapper: Wrapper },
    )

    act(() => {
      result.current.mutate({ id: 'abc' })
    })

    expect(stream.calls).toHaveLength(1)
  })
})

describe('useStreamingMutation — toBody transformer', () => {
  it('applies toBody before sending the request', () => {
    const stream = mockStream<string>('/static')
    const { Wrapper } = createQueryWrapper()
    const { result } = renderHook(
      () =>
        useStreamingMutation<{ id: string }, string>({
          path: '/static',
          toBody: (input) => ({ wrapped: input }),
        }),
      { wrapper: Wrapper },
    )

    act(() => {
      result.current.mutate({ id: 'abc' })
    })

    expect(stream.calls[0].body).toEqual({ wrapped: { id: 'abc' } })
  })

  it('passes the input through unchanged when no toBody is provided', () => {
    const stream = mockStream<string>('/static')
    const { Wrapper } = createQueryWrapper()
    const { result } = renderHook(
      () => useStreamingMutation<{ id: string }, string>({ path: '/static' }),
      { wrapper: Wrapper },
    )

    act(() => {
      result.current.mutate({ id: 'abc' })
    })

    expect(stream.calls[0].body).toEqual({ id: 'abc' })
  })
})

describe('useStreamingMutation — reset()', () => {
  it('clears data/error/progress/isPending back to idle', () => {
    const stream = mockStream<string>('/static')
    const { Wrapper } = createQueryWrapper()
    const { result } = renderHook(
      () => useStreamingMutation<{ id: string }, string>({ path: '/static' }),
      { wrapper: Wrapper },
    )

    act(() => {
      result.current.mutate({ id: 'x' })
    })
    act(() => {
      stream.complete('done')
    })
    expect(result.current.data).toBe('done')

    act(() => {
      result.current.reset()
    })

    expect(result.current.data).toBeUndefined()
    expect(result.current.error).toBeNull()
    expect(result.current.isPending).toBe(false)
    expect(result.current.progress).toBeNull()
  })
})

describe('useStreamingMutation — abort behavior', () => {
  it('aborts the previous in-flight controller when mutate() is called again', () => {
    const abortSpy = vi.spyOn(AbortController.prototype, 'abort')

    mockStream<string>('/static')
    const { Wrapper } = createQueryWrapper()
    const { result } = renderHook(
      () => useStreamingMutation<{ id: string }, string>({ path: '/static' }),
      { wrapper: Wrapper },
    )

    act(() => {
      result.current.mutate({ id: 'first' })
    })
    const callsBefore = abortSpy.mock.calls.length

    act(() => {
      result.current.mutate({ id: 'second' })
    })
    expect(abortSpy.mock.calls.length).toBeGreaterThan(callsBefore)

    abortSpy.mockRestore()
  })

  it('aborts the in-flight controller on unmount', () => {
    const abortSpy = vi.spyOn(AbortController.prototype, 'abort')

    mockStream<string>('/static')
    const { Wrapper } = createQueryWrapper()
    const { result, unmount } = renderHook(
      () => useStreamingMutation<{ id: string }, string>({ path: '/static' }),
      { wrapper: Wrapper },
    )

    act(() => {
      result.current.mutate({ id: 'x' })
    })
    const callsBefore = abortSpy.mock.calls.length

    unmount()
    expect(abortSpy.mock.calls.length).toBeGreaterThan(callsBefore)

    abortSpy.mockRestore()
  })
})

describe('useStreamingMutation — callbacks', () => {
  it('invokes onSuccess callback with the final data', () => {
    const stream = mockStream<string>('/static')
    const onSuccess = vi.fn()
    const { Wrapper } = createQueryWrapper()
    const { result } = renderHook(
      () => useStreamingMutation<{ id: string }, string>({ path: '/static' }),
      { wrapper: Wrapper },
    )

    act(() => {
      result.current.mutate({ id: 'x' }, { onSuccess })
    })
    act(() => {
      stream.complete('finished')
    })

    expect(onSuccess).toHaveBeenCalledWith('finished')
  })

  it('invokes onError callback with the error', () => {
    const stream = mockStream<string>('/static')
    const onError = vi.fn()
    const { Wrapper } = createQueryWrapper()
    const { result } = renderHook(
      () => useStreamingMutation<{ id: string }, string>({ path: '/static' }),
      { wrapper: Wrapper },
    )

    act(() => {
      result.current.mutate({ id: 'x' }, { onError })
    })
    const boom = new Error('boom')
    act(() => {
      stream.error(boom)
    })

    expect(onError).toHaveBeenCalledWith(boom)
  })
})
