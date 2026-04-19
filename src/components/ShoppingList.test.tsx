import { fireEvent, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { makeAggregatedIngredient, makeIngredientSource } from '../test/factories'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { renderWithProviders } from '../test/renderWithProviders'
import { ShoppingList } from './ShoppingList'

// Week of Monday 2026-04-20 through Sunday 2026-04-26.
const MONDAY = '2026-04-20'
const SUNDAY = '2026-04-26'
const SHOPPING_URL = `/api/shopping-list?start_date=${MONDAY}&end_date=${SUNDAY}`

beforeEach(() => {
  vi.useFakeTimers({ toFake: ['Date'] })
  vi.setSystemTime(new Date('2026-04-20T12:00:00'))
  installFetchMock()
})
afterEach(() => {
  resetFetchMock()
  vi.useRealTimers()
})

describe('ShoppingList', () => {
  it('renders aggregated ingredients returned by the server and expands sources on click', async () => {
    const tomato = makeAggregatedIngredient({
      ingredient_name: 'Tomato',
      total_amount: { type: 'single', value: 4 },
      total_unit: 'cups',
      items: [
        makeIngredientSource({
          source_name: 'Pasta',
          meal_date: MONDAY,
          meal_type: 'Dinner',
        }),
      ],
    })
    mockJson('GET', SHOPPING_URL, [tomato])

    renderWithProviders(<ShoppingList />)

    // Ingredient name + aggregated quantity render in the card header.
    await screen.findByText('Tomato')
    expect(screen.getByText(/4 cups/)).toBeInTheDocument()

    // Click the card header to expand sources; the recipe name appears.
    const toggle = screen.getByRole('button', { name: /Tomato/ })
    fireEvent.click(toggle)
    // Pasta appears in the "Mon Dinner — Pasta" source label.
    await waitFor(() => expect(screen.getByText(/Pasta/)).toBeInTheDocument())
  })

  it('shows the empty state when the server returns no ingredients', async () => {
    mockJson('GET', SHOPPING_URL, [])

    renderWithProviders(<ShoppingList />)

    await screen.findByText('Nothing to buy this week')
    expect(screen.getByText(/Plan some meals in the Planner tab/)).toBeInTheDocument()
  })

  it('refetches fresh aggregates when the shopping-list query is invalidated', async () => {
    // Initial state: one ingredient on the list.
    const tomato = makeAggregatedIngredient({ ingredient_name: 'Tomato' })
    mockJson('GET', SHOPPING_URL, [tomato])

    const { client } = renderWithProviders(<ShoppingList />)
    await screen.findByText('Tomato')

    // Simulate what happens after a meal is deleted elsewhere in the app:
    // the shopping-list cache is invalidated and the refetch returns empty.
    mockJson('GET', SHOPPING_URL, []) // shadow: later registration wins
    await client.invalidateQueries({ queryKey: ['shopping'] })

    await waitFor(() => expect(screen.queryByText('Tomato')).not.toBeInTheDocument())
    expect(screen.getByText('Nothing to buy this week')).toBeInTheDocument()
  })
})
