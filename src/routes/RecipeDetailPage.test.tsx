import type { QueryClient } from '@tanstack/react-query'
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react'
import { Route, Routes, useNavigate } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { RecipeManager } from '../components/RecipeManager'
import { makeRecipe } from '../test/factories'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { renderWithProviders } from '../test/renderWithProviders'
import type { Recipe } from '../types/recipe'
import { RecipeDetailPage } from './RecipeDetailPage'

// The real panel streams an adaptation from the AI endpoint. This stand-in
// hands the page a finished draft, so a test can reach adapt-edit mode.
vi.mock('../components/AdaptRecipePanel', () => ({
  AdaptRecipePanel: ({ onEdit }: { onEdit: (draft: unknown) => void }) => (
    <button
      type='button'
      onClick={() =>
        onEdit({
          name: 'Soup for Steve',
          source: 'adapted',
          parent_recipe_id: 'r2',
          servings: 2,
          instructions: 'Stir.',
          ingredients: [],
          tags: [],
        })}
    >
      Use adapted draft
    </button>
  ),
}))

function renderDetail(path = '/recipes/r1') {
  return renderWithProviders(
    <Routes>
      <Route path='/recipes' element={<RecipeManager />} />
      <Route path='/recipes/:id' element={<RecipeDetailPage />} />
    </Routes>,
    { initialPath: path },
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

// TanStack Query hands refetched data to React on a later timer tick, so an
// event fired straight after `refetchQueries` still runs against the old
// render. Waiting out that tick makes the page see the refetched recipe.
async function refetchAndRender(client: QueryClient, queryKey: string[]) {
  await act(async () => {
    await client.refetchQueries({ queryKey })
    await new Promise((resolve) => setTimeout(resolve, 0))
  })
}

function HistoryButtons() {
  const navigate = useNavigate()
  return (
    <>
      <button type='button' onClick={() => navigate('/recipes/soup')}>Go to soup</button>
      <button type='button' onClick={() => navigate(-1)}>Go back</button>
    </>
  )
}

// Render the detail page under buttons that move through history, starting on
// the pasta recipe.
function renderWithHistory() {
  return renderWithProviders(
    <>
      <HistoryButtons />
      <Routes>
        <Route path='/recipes/:id' element={<RecipeDetailPage />} />
      </Routes>
    </>,
    { initialPath: '/recipes/pasta' },
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
    // Instructions render through the lazy-loaded RecipeMarkdown.
    expect(await screen.findByText('Boil water, add pasta.')).toBeInTheDocument()
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
    // Untouched servings and ingredients stay out of the body.
    expect(putBody).not.toHaveProperty('servings')
    expect(putBody).not.toHaveProperty('ingredients')

    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['recipes'] })
  })

  describe('edit form ingredients payload', () => {
    // A Tomato amount of 2 keeps its input distinct from the servings input,
    // which shows 4.
    const tomatoRecipe = () =>
      makeRecipe({
        id: 'r1',
        ingredients: JSON.stringify([
          { name: 'Tomato', amount: { type: 'single', value: 2 }, unit: 'cups' },
        ]),
      })

    async function editAndSave(edit: () => void, recipe = tomatoRecipe()) {
      mockJson('GET', '/api/recipes/r1', recipe)
      renderDetail()
      await waitFor(() =>
        expect(screen.getByRole('heading', { name: 'Pasta' })).toBeInTheDocument()
      )
      fireEvent.click(screen.getByRole('button', { name: /Edit/ }))

      mockJson('PUT', '/api/recipes/r1', recipe)
      edit()
      fireEvent.click(screen.getByRole('button', { name: 'Save Changes' }))

      return { putBody: await sentPutBody() }
    }

    const rescaleWarning = () => screen.queryByText(/will not be rescaled/)

    it('a servings-only edit leaves ingredients out so the server rescales them', async () => {
      const { putBody } = await editAndSave(() => {
        fireEvent.change(screen.getByDisplayValue('4'), { target: { value: '6' } })
        expect(rescaleWarning()).not.toBeInTheDocument()
      })
      expect(putBody.servings).toBe(6)
      expect(putBody).not.toHaveProperty('ingredients')
    })

    it('an ingredient edit sends the full ingredient list', async () => {
      const { putBody } = await editAndSave(() => {
        fireEvent.change(screen.getByDisplayValue('4'), { target: { value: '6' } })
        fireEvent.change(screen.getByDisplayValue('2'), { target: { value: '3' } })
        expect(rescaleWarning()).toHaveTextContent(
          'You changed the ingredients, so their amounts are saved as entered and will not be rescaled to 6 servings.',
        )
      })
      expect(putBody.servings).toBe(6)
      expect(putBody.ingredients).toEqual([
        { name: 'Tomato', amount: { type: 'single', value: 3 }, unit: 'cups' },
      ])
    })

    it('the warning names a single serving in the singular', async () => {
      await editAndSave(() => {
        fireEvent.change(screen.getByDisplayValue('4'), { target: { value: '1' } })
        fireEvent.change(screen.getByDisplayValue('2'), { target: { value: '3' } })
        expect(rescaleWarning()).toHaveTextContent('will not be rescaled to 1 serving.')
      })
    })

    it('reverting the ingredient edit removes the warning and lets the server rescale', async () => {
      const { putBody } = await editAndSave(() => {
        fireEvent.change(screen.getByDisplayValue('4'), { target: { value: '6' } })
        fireEvent.change(screen.getByDisplayValue('2'), { target: { value: '3' } })
        expect(rescaleWarning()).toBeInTheDocument()
        fireEvent.change(screen.getByDisplayValue('3'), { target: { value: '2' } })
        expect(rescaleWarning()).not.toBeInTheDocument()
      })
      expect(putBody.servings).toBe(6)
      expect(putBody).not.toHaveProperty('ingredients')
    })

    it('reverting the servings change removes the warning', async () => {
      const { putBody } = await editAndSave(() => {
        fireEvent.change(screen.getByDisplayValue('2'), { target: { value: '3' } })
        expect(rescaleWarning()).not.toBeInTheDocument()
        fireEvent.change(screen.getByDisplayValue('4'), { target: { value: '6' } })
        expect(rescaleWarning()).toBeInTheDocument()
        fireEvent.change(screen.getByDisplayValue('6'), { target: { value: '4' } })
        expect(rescaleWarning()).not.toBeInTheDocument()
      })
      expect(putBody.servings).toBe(4)
      expect(putBody.ingredients).toEqual([
        { name: 'Tomato', amount: { type: 'single', value: 3 }, unit: 'cups' },
      ])
    })

    it('a row removed and retyped unchanged does not count as an edit', async () => {
      // The stored row carries `notes: null`, while a row added in the form
      // carries no notes at all.
      const recipe = makeRecipe({
        id: 'r1',
        ingredients: JSON.stringify([
          { name: 'Tomato', amount: { type: 'single', value: 2 }, unit: 'cups', notes: null },
        ]),
      })
      const { putBody } = await editAndSave(() => {
        fireEvent.change(screen.getByDisplayValue('4'), { target: { value: '6' } })
        const row = screen.getByDisplayValue('Tomato').parentElement!
        fireEvent.click(within(row).getAllByRole('button').at(-1)!)
        fireEvent.click(screen.getByRole('button', { name: '+ Add ingredient' }))
        fireEvent.change(screen.getByPlaceholderText('Amt'), { target: { value: '2' } })
        fireEvent.change(screen.getByPlaceholderText('Unit'), { target: { value: 'cups' } })
        fireEvent.change(screen.getByPlaceholderText('Ingredient name'), {
          target: { value: 'Tomato' },
        })
        expect(rescaleWarning()).not.toBeInTheDocument()
      }, recipe)
      expect(putBody.servings).toBe(6)
      expect(putBody).not.toHaveProperty('ingredients')
    })

    describe('after another client resizes the recipe during editing', () => {
      // The form stays open on 4 servings while a refetch brings in the same
      // recipe resized to 8.
      async function editResizeElsewhereAndSave(edit: () => void) {
        const recipe = tomatoRecipe()
        mockJson('GET', '/api/recipes/r1', recipe)
        const { client } = renderDetail()
        await waitFor(() =>
          expect(screen.getByRole('heading', { name: 'Pasta' })).toBeInTheDocument()
        )
        fireEvent.click(screen.getByRole('button', { name: /Edit/ }))

        const resized: Recipe = {
          ...recipe,
          servings: 8,
          ingredients: JSON.stringify([
            { name: 'Tomato', amount: { type: 'single', value: 4 }, unit: 'cups' },
          ]),
        }
        mockJson('GET', '/api/recipes/r1', resized)
        await refetchAndRender(client, ['recipes', 'r1'])

        mockJson('PUT', '/api/recipes/r1', resized)
        edit()
        fireEvent.click(screen.getByRole('button', { name: 'Save Changes' }))
        return sentPutBody()
      }

      it('an untouched count stays out, so the resize is not scaled back', async () => {
        const putBody = await editResizeElsewhereAndSave(() => {
          fireEvent.change(screen.getByDisplayValue('Pasta'), { target: { value: 'Pasta v2' } })
        })
        expect(putBody.name).toBe('Pasta v2')
        expect(putBody).not.toHaveProperty('servings')
        expect(putBody).not.toHaveProperty('ingredients')
      })

      it('an ingredients-only edit sends the count the list was sized for', async () => {
        const putBody = await editResizeElsewhereAndSave(() => {
          fireEvent.change(screen.getByDisplayValue('2'), { target: { value: '3' } })
        })
        expect(putBody.servings).toBe(4)
        expect(putBody.ingredients).toEqual([
          { name: 'Tomato', amount: { type: 'single', value: 3 }, unit: 'cups' },
        ])
      })
    })
  })

  it('a refetch during editing does not make untouched ingredients look edited', async () => {
    // The form keeps the ingredients it opened with. A background refetch
    // that changes the stored list must not turn those into an explicit
    // ingredients write, which would skip the server rescale.
    const recipe = makeRecipe({
      id: 'r1',
      ingredients: JSON.stringify([
        { name: 'Tomato', amount: { type: 'single', value: 2 }, unit: 'cups' },
      ]),
    })
    mockJson('GET', '/api/recipes/r1', recipe)
    const { client } = renderDetail()
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Pasta' })).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /Edit/ }))

    const changedElsewhere: Recipe = {
      ...recipe,
      ingredients: JSON.stringify([
        { name: 'Tomato', amount: { type: 'single', value: 7 }, unit: 'cups' },
      ]),
    }
    mockJson('GET', '/api/recipes/r1', changedElsewhere)
    await refetchAndRender(client, ['recipes', 'r1'])

    fireEvent.change(screen.getByDisplayValue('4'), { target: { value: '6' } })
    mockJson('PUT', '/api/recipes/r1', changedElsewhere)
    fireEvent.click(screen.getByRole('button', { name: 'Save Changes' }))

    const putBody = await sentPutBody()
    expect(putBody.servings).toBe(6)
    expect(putBody).not.toHaveProperty('ingredients')
  })

  it('history navigation while editing reseeds the form from the recipe it lands on', async () => {
    // Every recipe shares one mounted page, so an edit form opened on one
    // recipe must not be saved onto the recipe that Back lands on.
    const pasta = makeRecipe({ id: 'r1', slug: 'pasta', name: 'Pasta' })
    const soup = makeRecipe({ id: 'r2', slug: 'soup', name: 'Soup', servings: 2 })
    mockJson('GET', '/api/recipes/pasta', pasta)
    mockJson('GET', '/api/recipes/soup', soup)
    renderWithHistory()
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Pasta' })).toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: 'Go to soup' }))
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Soup' })).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /Edit/ }))
    fireEvent.click(screen.getByRole('button', { name: 'Go back' }))
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'Edit Pasta' })).toBeInTheDocument()
    )

    mockJson('PUT', '/api/recipes/r1', pasta)
    fireEvent.click(screen.getByRole('button', { name: 'Save Changes' }))

    const putBody = await sentPutBody()
    expect(putBody.name).toBe('Pasta')
    // The form reseeded from Pasta, so its untouched count is not sent.
    expect(putBody).not.toHaveProperty('servings')
  })

  it('a refetch after history navigation does not make untouched ingredients look edited', async () => {
    // A form that Back lands on was opened without the Edit button, so it
    // needs its own snapshot for a refetch to leave the list unedited.
    const tomatoes = (value: number) =>
      JSON.stringify([{ name: 'Tomato', amount: { type: 'single', value }, unit: 'cups' }])
    const pasta = makeRecipe({ id: 'r1', slug: 'pasta', name: 'Pasta', ingredients: tomatoes(2) })
    const soup = makeRecipe({ id: 'r2', slug: 'soup', name: 'Soup', servings: 2 })
    mockJson('GET', '/api/recipes/pasta', pasta)
    mockJson('GET', '/api/recipes/soup', soup)
    const { client } = renderWithHistory()
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Pasta' })).toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: 'Go to soup' }))
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Soup' })).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /Edit/ }))
    fireEvent.click(screen.getByRole('button', { name: 'Go back' }))
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'Edit Pasta' })).toBeInTheDocument()
    )
    await waitFor(() => expect(screen.getByDisplayValue('2')).toBeInTheDocument())

    const changedElsewhere: Recipe = { ...pasta, ingredients: tomatoes(7) }
    mockJson('GET', '/api/recipes/pasta', changedElsewhere)
    await refetchAndRender(client, ['recipes', 'pasta'])

    fireEvent.change(screen.getByDisplayValue('4'), { target: { value: '6' } })
    mockJson('PUT', '/api/recipes/r1', changedElsewhere)
    fireEvent.click(screen.getByRole('button', { name: 'Save Changes' }))

    const putBody = await sentPutBody()
    expect(putBody.servings).toBe(6)
    expect(putBody).not.toHaveProperty('ingredients')
  })

  it('an adapted draft never shows the rescale warning', async () => {
    // The draft has 2 servings and no ingredients, which differ from the
    // stored 4 servings and list. A new recipe has nothing to rescale.
    mockJson('GET', '/api/recipes/r1', makeRecipe({ id: 'r1' }))
    renderDetail()
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Pasta' })).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /Adapt/ }))
    fireEvent.click(screen.getByRole('button', { name: 'Use adapted draft' }))

    expect(screen.getByRole('heading', { name: 'Edit Adapted Recipe' })).toBeInTheDocument()
    expect(screen.queryByText(/will not be rescaled/)).not.toBeInTheDocument()
  })

  it('history navigation while editing an adapted draft drops the draft', async () => {
    // A draft adapted from Soup must not be offered for saving on the recipe
    // that Back lands on.
    const pasta = makeRecipe({ id: 'r1', slug: 'pasta', name: 'Pasta' })
    const soup = makeRecipe({ id: 'r2', slug: 'soup', name: 'Soup', servings: 2 })
    mockJson('GET', '/api/recipes/pasta', pasta)
    mockJson('GET', '/api/recipes/soup', soup)
    renderWithHistory()
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Pasta' })).toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: 'Go to soup' }))
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Soup' })).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: /Adapt/ }))
    fireEvent.click(screen.getByRole('button', { name: 'Use adapted draft' }))
    expect(screen.getByRole('heading', { name: 'Edit Adapted Recipe' })).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'Go back' }))
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Pasta' })).toBeInTheDocument())
    expect(screen.queryByRole('heading', { name: 'Edit Adapted Recipe' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Save Adapted Recipe' })).not.toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: 'Edit Pasta' })).not.toBeInTheDocument()

    // Returning to Soup does not bring the dropped draft back.
    fireEvent.click(screen.getByRole('button', { name: 'Go to soup' }))
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Soup' })).toBeInTheDocument())
    expect(screen.queryByRole('heading', { name: 'Edit Adapted Recipe' })).not.toBeInTheDocument()
  })

  describe('cooking mode', () => {
    // Cook mode auto-fetches enhanced instructions. Default to a benign
    // empty-string success so each test can opt into specific behavior
    // (or override with a later mockJson call).
    beforeEach(() => {
      mockJson('POST', '/api/recipes/r1/enhance', '')
    })

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

    it('fetches and renders enhanced instructions when entering cook mode', async () => {
      const pasta = makeRecipe({ id: 'r1', name: 'Pasta', instructions: 'Plain step.' })
      mockJson('GET', '/api/recipes/r1', pasta)
      // Enhanced version returns a punchier rewrite with markdown bold.
      mockJson('POST', '/api/recipes/r1/enhance', 'Bring water to a **rolling boil**.')

      renderDetail('/recipes/r1?mode=cook')

      await waitFor(() =>
        expect(screen.getByRole('heading', { level: 1, name: 'Pasta' })).toBeInTheDocument()
      )
      // Enhanced text wins out over the original instructions.
      await waitFor(() => expect(screen.queryByText('Plain step.')).not.toBeInTheDocument())
      const boldCallout = await screen.findByText('rolling boil')
      expect(boldCallout.tagName).toBe('STRONG')
    })

    it('falls back to plain instructions if the enhance request fails', async () => {
      const pasta = makeRecipe({ id: 'r1', name: 'Pasta', instructions: 'Plain step.' })
      mockJson('GET', '/api/recipes/r1', pasta)
      mockJson('POST', '/api/recipes/r1/enhance', { message: 'nope' }, { status: 500 })

      renderDetail('/recipes/r1?mode=cook')

      await waitFor(() =>
        expect(screen.getByRole('heading', { level: 1, name: 'Pasta' })).toBeInTheDocument()
      )
      // The plain instructions stay visible — failure is silent.
      expect(await screen.findByText('Plain step.')).toBeInTheDocument()
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
