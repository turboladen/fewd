import { fireEvent, screen, waitFor } from '@testing-library/react'
import { Route, Routes } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { renderWithProviders } from '../test/renderWithProviders'
import type { DrinkRecipe } from '../types/drinkRecipe'
import { DrinkRecipeDetailPage } from './DrinkRecipeDetailPage'

function makeDrink(overrides: Partial<DrinkRecipe> = {}): DrinkRecipe {
  return {
    id: 'd1',
    slug: 'negroni',
    name: 'Negroni',
    description: 'Bitter and bright',
    source: 'manual',
    source_url: null,
    servings: 1,
    instructions: 'Stir with ice.',
    ingredients: JSON.stringify([
      { name: 'gin', amount: { type: 'single', value: 1 }, unit: 'oz' },
    ]),
    technique: 'stirred',
    glassware: 'rocks',
    garnish: 'orange peel',
    tags: '[]',
    notes: 'Use Campari',
    icon: '🍸',
    is_favorite: false,
    is_non_alcoholic: false,
    rating: 4,
    times_made: 0,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
    ...overrides,
  }
}

function renderPage() {
  return renderWithProviders(
    <Routes>
      <Route path='/cocktails/recipes/:id' element={<DrinkRecipeDetailPage />} />
    </Routes>,
    { initialPath: '/cocktails/recipes/d1' },
  )
}

const isPut = ([, init]: Parameters<typeof fetch>) =>
  (init as RequestInit | undefined)?.method === 'PUT'

// Wait for the page to send its PUT, then return the parsed request body.
async function sentPutBody() {
  await waitFor(() => expect(vi.mocked(fetch).mock.calls.some(isPut)).toBe(true))
  const putCall = vi.mocked(fetch).mock.calls.find(isPut)
  return JSON.parse((putCall![1] as RequestInit).body as string)
}

beforeEach(() => installFetchMock())
afterEach(() => resetFetchMock())

describe('DrinkRecipeDetailPage', () => {
  it('an edit that empties fields set at load sends null so the server clears them', async () => {
    const drink = makeDrink()
    mockJson('GET', '/api/drink-recipes/d1', drink)
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: /Edit/ }))

    for (const value of ['Bitter and bright', 'stirred', 'rocks', 'orange peel', 'Use Campari']) {
      fireEvent.change(screen.getByDisplayValue(value), { target: { value: '' } })
    }
    mockJson('PUT', '/api/drink-recipes/d1', drink)
    fireEvent.click(screen.getByRole('button', { name: 'Save Changes' }))

    const putBody = await sentPutBody()
    for (const field of ['description', 'technique', 'glassware', 'garnish', 'notes']) {
      expect(putBody, field).toHaveProperty(field, null)
    }
  })

  it('ignores Escape while the edit form is saving', async () => {
    mockJson('GET', '/api/drink-recipes/d1', makeDrink())
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: /Edit/ }))

    // Hold the update open so Escape lands while it is in flight.
    vi.mocked(fetch).mockImplementationOnce(() => new Promise<Response>(() => {}))
    fireEvent.click(screen.getByRole('button', { name: 'Save Changes' }))
    await waitFor(() => expect(screen.getByRole('button', { name: 'Cancel' })).toBeDisabled())
    fireEvent.keyDown(window, { key: 'Escape' })

    expect(screen.getByRole('button', { name: 'Save Changes' })).toBeInTheDocument()
  })

  it('a field empty at load is left out of the update', async () => {
    const drink = makeDrink({
      description: null,
      technique: null,
      glassware: null,
      garnish: null,
      notes: null,
      icon: null,
    })
    mockJson('GET', '/api/drink-recipes/d1', drink)
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: /Edit/ }))

    fireEvent.change(screen.getByDisplayValue('Negroni'), { target: { value: 'Boulevardier' } })
    mockJson('PUT', '/api/drink-recipes/d1', drink)
    fireEvent.click(screen.getByRole('button', { name: 'Save Changes' }))

    const putBody = await sentPutBody()
    expect(putBody.name).toBe('Boulevardier')
    for (const field of ['description', 'technique', 'glassware', 'garnish', 'notes', 'icon']) {
      expect(putBody, field).not.toHaveProperty(field)
    }
  })

  it('submitting the form trims prep it was never blurred out of', async () => {
    const drink = makeDrink()
    mockJson('GET', '/api/drink-recipes/d1', drink)
    renderPage()
    fireEvent.click(await screen.findByRole('button', { name: /Edit/ }))

    fireEvent.change(screen.getByPlaceholderText('Prep (optional)'), {
      target: { value: ' chilled ' },
    })
    mockJson('PUT', '/api/drink-recipes/d1', drink)
    fireEvent.click(screen.getByRole('button', { name: 'Save Changes' }))

    const putBody = await sentPutBody()
    expect(putBody.ingredients[0].prep).toBe('chilled')
  })

  it('the clear-rating button sends a null rating', async () => {
    const drink = makeDrink()
    mockJson('GET', '/api/drink-recipes/d1', drink)
    renderPage()

    mockJson('PUT', '/api/drink-recipes/d1', { ...drink, rating: null })
    fireEvent.click(await screen.findByRole('button', { name: 'Clear rating' }))

    expect(await sentPutBody()).toEqual({ rating: null })
  })
})
