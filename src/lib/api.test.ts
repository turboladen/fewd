import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { api } from './api'

// The server answers 403 to a POST that is neither JSON nor marked with
// X-Requested-With, so these pin the headers each helper sends.
describe('api request headers', () => {
  beforeEach(() => {
    installFetchMock()
  })

  afterEach(() => {
    resetFetchMock()
  })

  function sentInit(): RequestInit {
    expect(fetch).toHaveBeenCalledTimes(1)
    return vi.mocked(fetch).mock.calls[0][1] as RequestInit
  }

  it('marks a GET and a DELETE with X-Requested-With', async () => {
    mockJson('GET', '/api/recipes', [])
    await api.get('/recipes')
    expect(sentInit()).toEqual({ method: 'GET', headers: { 'X-Requested-With': 'fetch' } })
    vi.mocked(fetch).mockClear()

    mockJson('DELETE', '/api/recipes/r1', null, { status: 204 })
    await api.delete('/recipes/r1')
    expect(sentInit()).toEqual({ method: 'DELETE', headers: { 'X-Requested-With': 'fetch' } })
  })

  it('marks a POST with no body and sends no content type', async () => {
    mockJson('POST', '/api/recipes/r1/favorite', {})
    await api.post('/recipes/r1/favorite')
    expect(sentInit()).toEqual({ method: 'POST', headers: { 'X-Requested-With': 'fetch' } })
  })

  it('adds a JSON content type to a POST or PUT with a body', async () => {
    mockJson('POST', '/api/recipes', {})
    await api.post('/recipes', { name: 'Toast' })
    expect(sentInit()).toEqual({
      method: 'POST',
      headers: { 'X-Requested-With': 'fetch', 'Content-Type': 'application/json' },
      body: '{"name":"Toast"}',
    })
    vi.mocked(fetch).mockClear()

    mockJson('PUT', '/api/recipes/r1', {})
    await api.put('/recipes/r1', { name: 'Toast' })
    expect(sentInit()).toMatchObject({
      method: 'PUT',
      headers: { 'X-Requested-With': 'fetch', 'Content-Type': 'application/json' },
    })
  })

  it('marks an upload and leaves its content type to the browser', async () => {
    mockJson('POST', '/api/recipes/import/file', {})
    await api.upload('/recipes/import/file', new File(['%PDF'], 'r.pdf'))
    const init = sentInit()
    expect(init.method).toBe('POST')
    expect(init.headers).toEqual({ 'X-Requested-With': 'fetch' })
    expect(init.body).toBeInstanceOf(FormData)
  })

  it('marks a streamed POST and sends its JSON body', () => {
    mockJson('POST', '/api/suggestions/ai', {})
    api.stream('/suggestions/ai', { n: 1 }, { onComplete: () => {}, onError: () => {} })
    const init = sentInit()
    expect(init).toMatchObject({
      method: 'POST',
      headers: { 'X-Requested-With': 'fetch', 'Content-Type': 'application/json' },
      body: '{"n":1}',
    })
    expect(init.signal).toBeInstanceOf(AbortSignal)
  })
})
