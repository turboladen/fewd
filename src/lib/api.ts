const BASE = '/api'

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message)
    this.name = 'ApiError'
  }
}

async function handleResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const body = await response.json().catch(() => ({ message: 'Unknown error' }))
    throw new ApiError(response.status, body.message || `HTTP ${response.status}`)
  }

  // 204 No Content
  if (response.status === 204) {
    return undefined as unknown as T
  }

  return response.json()
}

/** Progress event streamed from the backend during AI generation */
export interface StreamProgressEvent {
  phase: 'thinking' | 'generating'
  message: string
  tokens?: number
}

/** Callbacks for SSE stream consumption */
export interface StreamCallbacks<T> {
  onProgress?: (event: StreamProgressEvent) => void
  onComplete: (data: T) => void
  onError: (error: Error) => void
}

export const api = {
  get<T>(path: string): Promise<T> {
    return fetch(`${BASE}${path}`).then(handleResponse<T>)
  },

  post<T>(path: string, body?: unknown): Promise<T> {
    return fetch(`${BASE}${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: body !== undefined ? JSON.stringify(body) : undefined,
    }).then(handleResponse<T>)
  },

  put<T>(path: string, body?: unknown): Promise<T> {
    return fetch(`${BASE}${path}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: body !== undefined ? JSON.stringify(body) : undefined,
    }).then(handleResponse<T>)
  },

  delete(path: string): Promise<void> {
    return fetch(`${BASE}${path}`, { method: 'DELETE' }).then(handleResponse<void>)
  },

  upload<T>(path: string, file: File): Promise<T> {
    const formData = new FormData()
    formData.append('file', file)
    return fetch(`${BASE}${path}`, {
      method: 'POST',
      body: formData,
    }).then(handleResponse<T>)
  },

  /**
   * POST a JSON body and consume the response as an SSE stream.
   * Used for AI endpoints that stream progress events before returning the final result.
   * Returns an AbortController so the caller can cancel the stream.
   */
  stream<T>(path: string, body: unknown, callbacks: StreamCallbacks<T>): AbortController {
    const controller = new AbortController()

    fetch(`${BASE}${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
      signal: controller.signal,
    })
      .then(async (response) => {
        if (!response.ok) {
          const errBody = await response.json().catch(() => ({ message: 'Unknown error' }))
          throw new ApiError(response.status, errBody.message || `HTTP ${response.status}`)
        }

        const reader = response.body?.getReader()
        if (!reader) throw new Error('No response body')

        const decoder = new TextDecoder()
        let buffer = ''
        let currentEvent = ''
        let currentData = ''

        for (;;) {
          const { done, value } = await reader.read()
          if (done) break

          buffer += decoder.decode(value, { stream: true })

          // Process complete lines
          while (buffer.includes('\n')) {
            const newlineIdx = buffer.indexOf('\n')
            const line = buffer.slice(0, newlineIdx).replace(/\r$/, '')
            buffer = buffer.slice(newlineIdx + 1)

            if (line.startsWith('event: ')) {
              currentEvent = line.slice(7)
            } else if (line.startsWith('data: ')) {
              currentData = line.slice(6)
            } else if (line === '' && currentEvent) {
              // Blank line = end of SSE event
              if (currentData) {
                try {
                  const parsed = JSON.parse(currentData)
                  if (currentEvent === 'progress') {
                    callbacks.onProgress?.(parsed as StreamProgressEvent)
                  } else if (currentEvent === 'complete') {
                    callbacks.onComplete(parsed.data as T)
                  } else if (currentEvent === 'error') {
                    callbacks.onError(new Error(parsed.message || 'Unknown streaming error'))
                  }
                } catch {
                  // Ignore unparsable SSE data
                }
              }
              currentEvent = ''
              currentData = ''
            }
          }
        }
      })
      .catch((err) => {
        if (err instanceof Error && err.name === 'AbortError') return
        callbacks.onError(err instanceof Error ? err : new Error(String(err)))
      })

    return controller
  },
}
