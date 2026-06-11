import { fireEvent, screen, within } from '@testing-library/react'
import { Route, Routes } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { makeRecipe } from '../test/factories'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { renderWithProviders } from '../test/renderWithProviders'
import { RecipeDetailPage } from './RecipeDetailPage'

beforeEach(() => {
  installFetchMock()
})
afterEach(() => {
  resetFetchMock()
  vi.restoreAllMocks()
})

function renderDetail() {
  return renderWithProviders(
    <Routes>
      <Route path='/recipes/:id' element={<RecipeDetailPage />} />
    </Routes>,
    { initialPath: '/recipes/pasta' },
  )
}

describe('RecipeDetail print view', () => {
  it('invokes window.print when the Print button is clicked', async () => {
    // jsdom has no print implementation — stub it so the click is observable.
    const printSpy = vi.fn()
    vi.stubGlobal('print', printSpy)

    mockJson('GET', '/api/recipes/pasta', makeRecipe({ slug: 'pasta', name: 'Pasta' }))
    renderDetail()

    const printButton = await screen.findByRole('button', { name: /print/i })
    fireEvent.click(printButton)

    expect(printSpy).toHaveBeenCalledTimes(1)
  })

  it('renders the recipe content (title, ingredients, instructions) inside the print container', async () => {
    mockJson(
      'GET',
      '/api/recipes/pasta',
      makeRecipe({
        slug: 'pasta',
        name: 'Spaghetti Bolognese',
        instructions: 'Brown the beef, then simmer the sauce.',
        ingredients: JSON.stringify([
          { name: 'Spaghetti', amount: { type: 'single', value: 1 }, unit: 'lb' },
          { name: 'Ground beef', amount: { type: 'single', value: 2 }, unit: 'cups' },
        ]),
      }),
    )
    const { container } = renderDetail()

    // Instructions render through the lazily-loaded markdown chunk, so wait for
    // that body text before scoping assertions — the title alone is synchronous
    // and would race ahead of the chunk.
    await screen.findByText(/Brown the beef/)

    // The `.print-recipe` container is what the @media print stylesheet reveals;
    // everything the cook needs on paper must live inside it.
    const printRoot = container.querySelector('.print-recipe')
    expect(printRoot).not.toBeNull()
    const printScope = within(printRoot as HTMLElement)

    expect(printScope.getByRole('heading', { name: 'Spaghetti Bolognese' })).toBeInTheDocument()
    expect(printScope.getByRole('heading', { name: 'Ingredients' })).toBeInTheDocument()
    expect(printScope.getByText('Spaghetti')).toBeInTheDocument()
    expect(printScope.getByText('Ground beef')).toBeInTheDocument()
    expect(printScope.getByRole('heading', { name: 'Instructions' })).toBeInTheDocument()
    expect(printScope.getByText(/Brown the beef/)).toBeInTheDocument()
  })
})
