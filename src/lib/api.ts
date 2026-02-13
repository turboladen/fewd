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
    return fetch(`${BASE}${path}`, { method: 'DELETE' }).then(handleResponse)
  },

  upload<T>(path: string, file: File): Promise<T> {
    const formData = new FormData()
    formData.append('file', file)
    return fetch(`${BASE}${path}`, {
      method: 'POST',
      body: formData,
    }).then(handleResponse<T>)
  },
}
