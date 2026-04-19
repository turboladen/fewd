import { fireEvent, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { renderWithProviders } from '../test/renderWithProviders'
import type { BarItem, BarItemCategory } from '../types/barItem'
import { BAR_ITEM_CATEGORIES, COMMON_BAR_ITEMS } from '../types/barItem'
import { BarInventory } from './BarInventory'

function makeBarItem(overrides: Partial<BarItem> = {}): BarItem {
  return {
    id: 'bi-1',
    name: 'Bourbon',
    category: 'spirit',
    created_at: '2026-04-01T00:00:00Z',
    ...overrides,
  }
}

const categoryMeta = (value: BarItemCategory) => BAR_ITEM_CATEGORIES.find((c) => c.value === value)!

beforeEach(() => installFetchMock())
afterEach(() => resetFetchMock())

describe('BarInventory', () => {
  it('renders each category tile with its emoji and applies the per-category tag color', async () => {
    const bourbon = makeBarItem({ id: 'bi-1', name: 'Bourbon', category: 'spirit' })
    const tonic = makeBarItem({ id: 'bi-2', name: 'Tonic Water', category: 'mixer' })
    mockJson('GET', '/api/bar-items', [bourbon, tonic])

    renderWithProviders(<BarInventory />)

    await waitFor(() => expect(screen.getByText('Bourbon')).toBeInTheDocument())

    // Each rendered category tile shows its emoji and the item's tag uses the
    // category's tagColor class — covers "correct emoji + per-category tag colors".
    const spiritMeta = categoryMeta('spirit')
    const mixerMeta = categoryMeta('mixer')
    expect(screen.getByText(spiritMeta.emoji)).toBeInTheDocument()
    expect(screen.getByText(mixerMeta.emoji)).toBeInTheDocument()

    const bourbonTag = screen.getByText('Bourbon').closest('span')
    const tonicTag = screen.getByText('Tonic Water').closest('span')
    expect(bourbonTag?.className).toContain('bg-amber-50')
    expect(tonicTag?.className).toContain('bg-teal-50')
  })

  it('Add-to-Bar: POSTs the new item and refetches so the tile appears', async () => {
    mockJson('GET', '/api/bar-items', [])
    renderWithProviders(<BarInventory />)

    // Empty state first
    await waitFor(() => expect(screen.getByText('Bar is empty')).toBeInTheDocument())

    // Stage the POST response + the next GET (last-registered wins on refetch).
    const gin = makeBarItem({ id: 'bi-new', name: 'Gin', category: 'spirit' })
    mockJson('POST', '/api/bar-items', gin, { status: 201 })
    mockJson('GET', '/api/bar-items', [gin])

    fireEvent.change(screen.getByPlaceholderText('Item name'), { target: { value: 'Gin' } })
    fireEvent.click(screen.getByRole('button', { name: /^Add$/ }))

    await waitFor(() => expect(screen.getByText('Gin')).toBeInTheDocument())
    expect(screen.queryByText('Bar is empty')).not.toBeInTheDocument()
  })

  it('Quick-Add: bulk-POSTs the category list and dedupes (case-insensitive) against existing items', async () => {
    // Pre-seed with one spirit that should be skipped by dedup.
    const existing = makeBarItem({ id: 'bi-0', name: 'bourbon', category: 'spirit' })
    mockJson('GET', '/api/bar-items', [existing])

    renderWithProviders(<BarInventory />)
    await waitFor(() => expect(screen.getByText('bourbon')).toBeInTheDocument())

    // Capture the POST body so we can assert dedup.
    let postedBody: unknown
    const origFetch = global.fetch
    global.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      if (init?.method === 'POST' && String(input) === '/api/bar-items/bulk') {
        postedBody = init.body ? JSON.parse(init.body as string) : null
      }
      return origFetch(input, init)
    }) as typeof fetch

    // Stage server response + refetch payload.
    const newItems = COMMON_BAR_ITEMS.spirit
      .filter((n) => n.toLowerCase() !== 'bourbon')
      .map((name, i) => makeBarItem({ id: `bi-s${i}`, name, category: 'spirit' }))
    mockJson('POST', '/api/bar-items/bulk', newItems, { status: 201 })
    mockJson('GET', '/api/bar-items', [existing, ...newItems])

    // Click the Quick-Add Spirits button (scope to the Quick Add card to disambiguate).
    const quickAddCard = screen.getByRole('heading', { name: 'Quick Add' })
      .parentElement as HTMLElement
    fireEvent.click(within(quickAddCard).getByRole('button', { name: /Spirits/ }))

    // A newly added item appears; the dedup victim (bourbon) is unchanged.
    await waitFor(() => expect(screen.getByText('Gin')).toBeInTheDocument())

    // Bulk payload excludes the existing 'bourbon' entry.
    const body = postedBody as { items: Array<{ name: string }> }
    const names = body.items.map((i) => i.name.toLowerCase())
    expect(names).not.toContain('bourbon')
    expect(body.items.length).toBe(COMMON_BAR_ITEMS.spirit.length - 1)
  })

  it('delete: clicking × on a tile removes the item via DELETE and refetches', async () => {
    const gin = makeBarItem({ id: 'bi-gin', name: 'Gin', category: 'spirit' })
    const tonic = makeBarItem({ id: 'bi-tonic', name: 'Tonic Water', category: 'mixer' })
    mockJson('GET', '/api/bar-items', [gin, tonic])

    renderWithProviders(<BarInventory />)
    await waitFor(() => expect(screen.getByText('Gin')).toBeInTheDocument())

    mockJson('DELETE', '/api/bar-items/bi-gin', null, { status: 204 })
    mockJson('GET', '/api/bar-items', [tonic]) // post-delete refetch

    fireEvent.click(screen.getByRole('button', { name: 'Remove Gin' }))

    await waitFor(() => expect(screen.queryByText('Gin')).not.toBeInTheDocument())
    expect(screen.getByText('Tonic Water')).toBeInTheDocument()
  })

  it('Clear All: two-step confirmation then DELETE /bar-items/all reveals the empty state', async () => {
    const gin = makeBarItem({ id: 'bi-gin', name: 'Gin', category: 'spirit' })
    mockJson('GET', '/api/bar-items', [gin])

    renderWithProviders(<BarInventory />)
    await waitFor(() => expect(screen.getByText('Gin')).toBeInTheDocument())

    // First click reveals the confirmation prompt, no request yet.
    fireEvent.click(screen.getByRole('button', { name: /Clear All/ }))
    expect(screen.getByText('Clear all?')).toBeInTheDocument()

    mockJson('DELETE', '/api/bar-items/all', null, { status: 204 })
    mockJson('GET', '/api/bar-items', []) // refetch returns empty

    fireEvent.click(screen.getByRole('button', { name: 'Yes' }))

    await waitFor(() => expect(screen.getByText('Bar is empty')).toBeInTheDocument())
    expect(screen.queryByText('Gin')).not.toBeInTheDocument()
  })
})
