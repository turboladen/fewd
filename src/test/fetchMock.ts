import { vi } from 'vitest'

type Route = { method: string; url: string; status: number; body: unknown }

let routes: Route[] = []
let spy: ReturnType<typeof vi.spyOn> | null = null

/**
 * Install a `fetch` spy for the current test. Call in `beforeEach`.
 *
 * Pairs with `resetFetchMock()` in `afterEach`. Does not auto-install —
 * each test file opts in so transitive imports don't accidentally stub
 * `fetch` in tests that expect the real one.
 */
export function installFetchMock(): void {
  routes = []
  spy = vi.spyOn(global, 'fetch').mockImplementation(async (input, init) => {
    const url = typeof input === 'string'
      ? input
      : input instanceof URL
      ? input.toString()
      : input.url
    const method = (init?.method ?? 'GET').toUpperCase()
    // Scan from the end so later registrations shadow earlier ones —
    // lets tests stage a post-mutation refetch with a different body.
    let route: Route | undefined
    for (let i = routes.length - 1; i >= 0; i--) {
      const r = routes[i]
      if (r.method === method && r.url === url) {
        route = r
        break
      }
    }
    if (!route) {
      const known = routes.map(r => `${r.method} ${r.url}`).join(', ') || '(none)'
      throw new Error(`No mock for ${method} ${url}. Registered: ${known}`)
    }
    return new Response(route.status === 204 ? null : JSON.stringify(route.body), {
      status: route.status,
      headers: { 'Content-Type': 'application/json' },
    })
  })
}

export function resetFetchMock(): void {
  spy?.mockRestore()
  spy = null
  routes = []
}

/**
 * Register a JSON response for an exact method + URL match.
 * URLs must include the `/api` prefix (api.ts prepends it on the real call).
 */
export function mockJson(
  method: string,
  url: string,
  body: unknown,
  options: { status?: number } = {},
): void {
  routes.push({ method: method.toUpperCase(), url, body, status: options.status ?? 200 })
}
