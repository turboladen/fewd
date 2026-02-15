import { useCallback, useRef, useState } from 'react'
import { api, type StreamProgressEvent } from '../lib/api'

export type { StreamProgressEvent }

interface StreamingMutationOptions<TInput> {
  /** API path — e.g. '/suggestions/ai' */
  path: string | ((input: TInput) => string)
  /** Transform input into the JSON body (defaults to identity) */
  toBody?: (input: TInput) => unknown
}

interface MutateCallbacks<TResult> {
  onSuccess?: (data: TResult) => void
  onError?: (error: Error) => void
}

interface StreamingMutationResult<TInput, TResult> {
  /** Call to start the streaming mutation */
  mutate: (input: TInput, callbacks?: MutateCallbacks<TResult>) => void
  /** The final result once complete */
  data: TResult | undefined
  /** Any error that occurred */
  error: Error | null
  /** Whether a request is in-flight */
  isPending: boolean
  /** Current progress event from the stream (null when idle or complete) */
  progress: StreamProgressEvent | null
  /** Reset state back to idle */
  reset: () => void
}

/**
 * A hook that mirrors useMutation but uses SSE streaming for progress events.
 * Drop-in replacement for useMutation on AI endpoints that now return SSE.
 */
export function useStreamingMutation<TInput, TResult>(
  options: StreamingMutationOptions<TInput>,
): StreamingMutationResult<TInput, TResult> {
  const [data, setData] = useState<TResult | undefined>(undefined)
  const [error, setError] = useState<Error | null>(null)
  const [isPending, setIsPending] = useState(false)
  const [progress, setProgress] = useState<StreamProgressEvent | null>(null)

  const controllerRef = useRef<AbortController | null>(null)

  const reset = useCallback(() => {
    controllerRef.current?.abort()
    controllerRef.current = null
    setData(undefined)
    setError(null)
    setIsPending(false)
    setProgress(null)
  }, [])

  const mutate = useCallback(
    (input: TInput, callbacks?: MutateCallbacks<TResult>) => {
      // Abort any in-flight request
      controllerRef.current?.abort()

      setData(undefined)
      setError(null)
      setIsPending(true)
      setProgress(null)

      const path = typeof options.path === 'function' ? options.path(input) : options.path
      const body = options.toBody ? options.toBody(input) : input

      const controller = api.stream<TResult>(path, body, {
        onProgress: (event) => {
          setProgress(event)
        },
        onComplete: (result) => {
          setData(result)
          setIsPending(false)
          setProgress(null)
          controllerRef.current = null
          callbacks?.onSuccess?.(result)
        },
        onError: (err) => {
          setError(err)
          setIsPending(false)
          setProgress(null)
          controllerRef.current = null
          callbacks?.onError?.(err)
        },
      })

      controllerRef.current = controller
    },
    [options],
  )

  return { mutate, data, error, isPending, progress, reset }
}
