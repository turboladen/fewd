import { type MockInstance, vi } from 'vitest'
import { api, type StreamCallbacks, type StreamProgressEvent } from '../lib/api'

export interface StreamHandle<T> {
  /** Drive a progress event into the live stream. */
  emit: (event: StreamProgressEvent) => void
  /** Resolve the stream with a final result. */
  complete: (data: T) => void
  /** Fail the stream. */
  error: (err: Error) => void
  /** Bodies of requests routed to this path (one entry per `api.stream` call). */
  calls: Array<{ body: unknown }>
}

interface InternalHandle {
  callbacks: StreamCallbacks<unknown> | null
  calls: Array<{ body: unknown }>
}

let handlers: Map<string, InternalHandle> | null = null
let spy: MockInstance | null = null

/**
 * Spy on `api.stream` so tests can drive SSE callbacks synchronously.
 * Call in `beforeEach`; pair with `resetStreamMock()` in `afterEach`.
 *
 * Why spy rather than fake a ReadableStream? The SSE parser in `api.stream`
 * is already exercised by every streaming feature in prod. Spying keeps
 * the `useStreamingMutation` state machine under test without duplicating
 * the parser in the mock.
 */
export function installStreamMock(): void {
  handlers = new Map()
  spy = vi.spyOn(api, 'stream').mockImplementation((path, body, callbacks) => {
    const h = handlers!.get(path)
    if (!h) {
      const known = [...handlers!.keys()].join(', ') || '(none)'
      throw new Error(`No mock for api.stream(${path}). Registered: ${known}`)
    }
    h.callbacks = callbacks as StreamCallbacks<unknown>
    h.calls.push({ body })
    return new AbortController()
  })
}

export function resetStreamMock(): void {
  spy?.mockRestore()
  spy = null
  handlers = null
}

/**
 * Register a stream mock for `path` and return a handle that can drive
 * progress/complete/error events. The `emit`/`complete`/`error` calls
 * throw if invoked before the component under test has called
 * `api.stream(path, ...)`.
 */
export function mockStream<T>(path: string): StreamHandle<T> {
  if (!handlers) throw new Error('mockStream() called before installStreamMock()')
  const internal: InternalHandle = { callbacks: null, calls: [] }
  handlers.set(path, internal)

  const ensureActive = (method: string) => {
    if (!internal.callbacks) {
      throw new Error(
        `streamMock(${path}).${method}() called before the component triggered api.stream`,
      )
    }
  }

  return {
    emit: (event) => {
      ensureActive('emit')
      internal.callbacks!.onProgress?.(event)
    },
    complete: (data) => {
      ensureActive('complete')
      ;(internal.callbacks!.onComplete as (d: T) => void)(data)
    },
    error: (err) => {
      ensureActive('error')
      internal.callbacks!.onError(err)
    },
    calls: internal.calls,
  }
}
