import { fireEvent, screen, waitFor } from '@testing-library/react'
import { Route, Routes } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { RecipeManager } from '../components/RecipeManager'
import { makeRecipe } from '../test/factories'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { renderWithProviders } from '../test/renderWithProviders'
import type { Recipe } from '../types/recipe'
import { RecipeDetailPage } from './RecipeDetailPage'

function renderDetail(path = '/recipes/r1') {
  return renderWithProviders(
    <Routes>
      <Route path='/recipes' element={<RecipeManager />} />
      <Route path='/recipes/:id' element={<RecipeDetailPage />} />
    </Routes>,
    { initialPath: path },
  )
}

beforeEach(() => installFetchMock())
afterEach(() => resetFetchMock())

describe('RecipeDetailPage', () => {
  it('renders the recipe name, ingredients, and instructions from the id-based URL', async () => {
    const pasta = makeRecipe({
      id: 'r1',
      name: 'Pasta',
      instructions: 'Boil water, add pasta.',
    })
    mockJson('GET', '/api/recipes/r1', pasta)

    renderDetail()

    await waitFor(() => expect(screen.getByRole('heading', { name: 'Pasta' })).toBeInTheDocument())
    // Factory default includes "Tomato" in ingredients.
    expect(screen.getByText('Tomato')).toBeInTheDocument()
    expect(screen.getByText('Boil water, add pasta.')).toBeInTheDocument()
    // Structural headings confirm we're in RecipeDetail, not the fallback.
    expect(screen.getByRole('heading', { name: 'Ingredients' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Instructions' })).toBeInTheDocument()
  })

  it('resolves a recipe from a slug-based URL', async () => {
    // Backend accepts either a UUID or a slug on the recipes/:id_or_slug route.
    // This test shadows that: the page URL is /recipes/pasta and the fetch fires
    // against /api/recipes/pasta — verifying end-to-end slug resolution.
    const pasta = makeRecipe({ id: 'r1', slug: 'pasta', name: 'Pasta' })
    mockJson('GET', '/api/recipes/pasta', pasta)

    renderDetail('/recipes/pasta')

    await waitFor(() => expect(screen.getByRole('heading', { name: 'Pasta' })).toBeInTheDocument())
  })

  it('toggling favorite POSTs and invalidates the recipes cache', async () => {
    const pasta = makeRecipe({ id: 'r1', name: 'Pasta', is_favorite: false })
    mockJson('GET', '/api/recipes/r1', pasta)

    const { client } = renderDetail()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Add to favorites' })).toBeInTheDocument()
    )

    // Stage the favorite POST + shadow the detail refetch with the flipped state.
    const favorited: Recipe = { ...pasta, is_favorite: true }
    mockJson('POST', '/api/recipes/r1/favorite', favorited)
    mockJson('GET', '/api/recipes/r1', favorited)

    fireEvent.click(screen.getByRole('button', { name: 'Add to favorites' }))

    // Behavior: the aria-label flips once the refetch lands.
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Remove from favorites' })).toBeInTheDocument()
    )
    // Contract: canonical query key invalidated.
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['recipes'] })
  })

  it('edit flow PUTs the updated fields and returns to view mode', async () => {
    const pasta = makeRecipe({ id: 'r1', name: 'Pasta', instructions: 'Boil water.' })
    mockJson('GET', '/api/recipes/r1', pasta)

    const { client } = renderDetail()
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    await waitFor(() => expect(screen.getByRole('heading', { name: 'Pasta' })).toBeInTheDocument())

    // Enter edit mode.
    fireEvent.click(screen.getByRole('button', { name: /Edit/ }))

    // The form's labels are not programmatically tied to inputs, so match by
    // the initial display value instead of accessible name.
    const nameInput = screen.getByDisplayValue('Pasta')
    fireEvent.change(nameInput, { target: { value: 'Pasta v2' } })

    const updated: Recipe = { ...pasta, name: 'Pasta v2' }
    mockJson('PUT', '/api/recipes/r1', updated)
    mockJson('GET', '/api/recipes/r1', updated)

    fireEvent.click(screen.getByRole('button', { name: 'Save Changes' }))

    // View mode renders the new name (form is gone).
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'Pasta v2' })).toBeInTheDocument()
    )
    expect(screen.queryByRole('button', { name: 'Save Changes' })).not.toBeInTheDocument()

    // Server saw the PUT with the renamed body.
    const putCall = vi.mocked(fetch).mock.calls.find(([, init]) =>
      (init as RequestInit | undefined)?.method === 'PUT'
    )
    expect(putCall).toBeDefined()
    const putBody = JSON.parse((putCall![1] as RequestInit).body as string)
    expect(putBody.name).toBe('Pasta v2')

    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['recipes'] })
  })

  describe('cooking mode', () => {
    it('renders CookingView (not the standard detail view) when ?mode=cook is set', async () => {
      const pasta = makeRecipe({ id: 'r1', name: 'Pasta', instructions: 'Boil.\nAdd pasta.' })
      mockJson('GET', '/api/recipes/r1', pasta)

      renderDetail('/recipes/r1?mode=cook')

      // CookingView uses h1 for the recipe name; RecipeDetail uses h2.
      await waitFor(() =>
        expect(screen.getByRole('heading', { level: 1, name: 'Pasta' })).toBeInTheDocument()
      )
      // The detail-view edit button must be absent — that's the whole point of cook mode.
      expect(screen.queryByRole('button', { name: /^Edit$/ })).not.toBeInTheDocument()
      // The exit affordance is visible.
      expect(screen.getByRole('button', { name: /Exit cooking mode/i })).toBeInTheDocument()
    })

    it('"Cook this" button on the detail view enters cook mode', async () => {
      const pasta = makeRecipe({ id: 'r1', name: 'Pasta' })
      mockJson('GET', '/api/recipes/r1', pasta)

      renderDetail()

      await waitFor(() =>
        expect(screen.getByRole('heading', { level: 2, name: 'Pasta' })).toBeInTheDocument()
      )

      fireEvent.click(screen.getByRole('button', { name: /Cook this/i }))

      // Now in cook mode: the heading promotes to h1, edit button is gone.
      await waitFor(() =>
        expect(screen.getByRole('heading', { level: 1, name: 'Pasta' })).toBeInTheDocument()
      )
      expect(screen.queryByRole('button', { name: /^Edit$/ })).not.toBeInTheDocument()
    })

    it('"Exit cooking mode" button returns to the standard detail view', async () => {
      const pasta = makeRecipe({ id: 'r1', name: 'Pasta' })
      mockJson('GET', '/api/recipes/r1', pasta)

      renderDetail('/recipes/r1?mode=cook')

      await waitFor(() =>
        expect(screen.getByRole('heading', { level: 1, name: 'Pasta' })).toBeInTheDocument()
      )

      fireEvent.click(screen.getByRole('button', { name: /Exit cooking mode/i }))

      // Detail view's h2 returns; cook-mode exit button is gone.
      await waitFor(() =>
        expect(screen.getByRole('heading', { level: 2, name: 'Pasta' })).toBeInTheDocument()
      )
      expect(screen.queryByRole('button', { name: /Exit cooking mode/i })).not.toBeInTheDocument()
    })

    it('Escape key while in cook mode returns to the standard detail view', async () => {
      const pasta = makeRecipe({ id: 'r1', name: 'Pasta' })
      mockJson('GET', '/api/recipes/r1', pasta)

      renderDetail('/recipes/r1?mode=cook')

      await waitFor(() =>
        expect(screen.getByRole('heading', { level: 1, name: 'Pasta' })).toBeInTheDocument()
      )

      fireEvent.keyDown(window, { key: 'Escape' })

      await waitFor(() =>
        expect(screen.getByRole('heading', { level: 2, name: 'Pasta' })).toBeInTheDocument()
      )
    })

    it('Escape exits cook mode first when a delete confirmation is pending underneath', async () => {
      const pasta = makeRecipe({ id: 'r1', name: 'Pasta' })
      mockJson('GET', '/api/recipes/r1', pasta)

      renderDetail()
      await waitFor(() =>
        expect(screen.getByRole('heading', { level: 2, name: 'Pasta' })).toBeInTheDocument()
      )

      // Stage delete confirmation, then enter cook mode while it's still up.
      fireEvent.click(screen.getByRole('button', { name: /Delete/ }))
      expect(screen.getByRole('button', { name: 'Yes' })).toBeInTheDocument()
      fireEvent.click(screen.getByRole('button', { name: /Cook this/i }))
      await waitFor(() =>
        expect(screen.getByRole('heading', { level: 1, name: 'Pasta' })).toBeInTheDocument()
      )

      // First Escape exits cook mode (priority over delete confirmation).
      fireEvent.keyDown(window, { key: 'Escape' })
      await waitFor(() =>
        expect(screen.getByRole('heading', { level: 2, name: 'Pasta' })).toBeInTheDocument()
      )
      // Confirmation state survived; second Escape cancels it.
      expect(screen.getByRole('button', { name: 'Yes' })).toBeInTheDocument()
      fireEvent.keyDown(window, { key: 'Escape' })
      expect(screen.queryByRole('button', { name: 'Yes' })).not.toBeInTheDocument()
    })
  })

  it('deleting a recipe navigates back to the list view', async () => {
    const pasta = makeRecipe({ id: 'r1', name: 'Pasta' })
    mockJson('GET', '/api/recipes/r1', pasta)
    // Destination render after navigate('/recipes').
    mockJson('GET', '/api/recipes', [])

    renderDetail()

    await waitFor(() => expect(screen.getByRole('heading', { name: 'Pasta' })).toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: /Delete/ }))
    // Confirmation state — now "Yes" / "No" buttons appear.
    mockJson('DELETE', '/api/recipes/r1', null, { status: 204 })
    fireEvent.click(screen.getByRole('button', { name: 'Yes' }))

    // RecipeManager renders the empty-state copy on the destination route.
    await waitFor(() => expect(screen.getByText('Your recipe book is empty')).toBeInTheDocument())
  })
})
