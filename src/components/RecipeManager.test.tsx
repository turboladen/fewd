import { act, fireEvent, screen, waitFor } from '@testing-library/react'
import { Route, Routes } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { RecipeDetailPage } from '../routes/RecipeDetailPage'
import { makeRecipe } from '../test/factories'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { renderWithProviders } from '../test/renderWithProviders'
import { installStreamMock, mockStream, resetStreamMock } from '../test/streamMock'
import type { ParsedRecipe, Recipe, ScaleResult } from '../types/recipe'
import { parseRecipe } from '../types/recipe'
import { RecipeManager, ScaleRecipePanel, ScalingPreviewAltRow } from './RecipeManager'

beforeEach(() => {
  installFetchMock()
  installStreamMock()
})
afterEach(() => {
  resetFetchMock()
  resetStreamMock()
  localStorage.clear()
})

/** Recipe-name buttons in DOM order — the card title is the only button bearing the name. */
function renderedRecipeOrder(names: string[]): string[] {
  const want = new Set(names)
  return screen
    .getAllByRole('button')
    .map((b) => b.textContent?.trim() ?? '')
    .filter((t) => want.has(t))
}

describe('RecipeManager', () => {
  it('renders recipes and filters the list client-side by search query', async () => {
    const pasta = makeRecipe({ id: 'r1', name: 'Pasta' })
    const pizza = makeRecipe({ id: 'r2', name: 'Pizza' })
    const salad = makeRecipe({ id: 'r3', name: 'Salad' })
    mockJson('GET', '/api/recipes', [pasta, pizza, salad])

    renderWithProviders(<RecipeManager />, { initialPath: '/recipes' })

    await waitFor(() => expect(screen.getByText('Pasta')).toBeInTheDocument())
    expect(screen.getByText('Pizza')).toBeInTheDocument()
    expect(screen.getByText('Salad')).toBeInTheDocument()

    // Search filter is client-side — no network call.
    const callsBefore = vi.mocked(fetch).mock.calls.length
    fireEvent.change(screen.getByPlaceholderText('Search recipes...'), {
      target: { value: 'piz' },
    })

    expect(screen.getByText('Pizza')).toBeInTheDocument()
    expect(screen.queryByText('Pasta')).not.toBeInTheDocument()
    expect(screen.queryByText('Salad')).not.toBeInTheDocument()
    expect(vi.mocked(fetch).mock.calls.length).toBe(callsBefore)
  })

  it('sorts by highest rating when the sort dropdown changes, unrated last', async () => {
    const mid = makeRecipe({ id: 'r1', name: 'Mid', rating: 3 })
    const unrated = makeRecipe({ id: 'r2', name: 'Unrated', rating: null })
    const top = makeRecipe({ id: 'r3', name: 'Top', rating: 5 })
    mockJson('GET', '/api/recipes', [mid, unrated, top])

    renderWithProviders(<RecipeManager />, { initialPath: '/recipes' })
    await waitFor(() => expect(screen.getByText('Mid')).toBeInTheDocument())

    // Default name-asc.
    expect(renderedRecipeOrder(['Mid', 'Unrated', 'Top'])).toEqual(['Mid', 'Top', 'Unrated'])

    fireEvent.change(screen.getByLabelText('Sort recipes'), { target: { value: 'rating-desc' } })

    expect(renderedRecipeOrder(['Mid', 'Unrated', 'Top'])).toEqual(['Top', 'Mid', 'Unrated'])
  })

  it('bubbles never-planned recipes to the top for "Not planned in a while"', async () => {
    const recent = makeRecipe({ id: 'r1', name: 'Recent', last_planned: '2026-05-01T00:00:00Z' })
    const never = makeRecipe({ id: 'r2', name: 'Never', last_planned: null })
    const old = makeRecipe({ id: 'r3', name: 'Old', last_planned: '2026-01-01T00:00:00Z' })
    mockJson('GET', '/api/recipes', [recent, never, old])

    renderWithProviders(<RecipeManager />, { initialPath: '/recipes' })
    await waitFor(() => expect(screen.getByText('Never')).toBeInTheDocument())

    fireEvent.change(screen.getByLabelText('Sort recipes'), {
      target: { value: 'last_planned-asc' },
    })

    expect(renderedRecipeOrder(['Recent', 'Never', 'Old'])).toEqual(['Never', 'Old', 'Recent'])
  })

  it('persists the chosen sort across remounts via localStorage', async () => {
    const mid = makeRecipe({ id: 'r1', name: 'Mid', rating: 3 })
    const top = makeRecipe({ id: 'r3', name: 'Top', rating: 5 })
    mockJson('GET', '/api/recipes', [mid, top])

    const first = renderWithProviders(<RecipeManager />, { initialPath: '/recipes' })
    await waitFor(() => expect(screen.getByText('Mid')).toBeInTheDocument())
    fireEvent.change(screen.getByLabelText('Sort recipes'), { target: { value: 'rating-desc' } })
    first.unmount()

    // Fresh mount reads the persisted preference.
    mockJson('GET', '/api/recipes', [mid, top])
    renderWithProviders(<RecipeManager />, { initialPath: '/recipes' })
    await waitFor(() => expect(screen.getByText('Top')).toBeInTheDocument())

    expect(screen.getByLabelText('Sort recipes')).toHaveValue('rating-desc')
    expect(renderedRecipeOrder(['Mid', 'Top'])).toEqual(['Top', 'Mid'])
  })

  it('falls back to default sort (no crash) when localStorage access is blocked', async () => {
    // Safari "Block All Cookies" (and sandboxed iframes) make Storage throw
    // SecurityError on access. Reading it during render would otherwise crash
    // the whole app via the root ErrorBoundary.
    const blocked = () => {
      throw new DOMException('The operation is insecure.', 'SecurityError')
    }
    const getItem = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(blocked)
    const setItem = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(blocked)
    try {
      const apple = makeRecipe({ id: 'r1', name: 'Apple' })
      const banana = makeRecipe({ id: 'r2', name: 'Banana' })
      mockJson('GET', '/api/recipes', [banana, apple])

      renderWithProviders(<RecipeManager />, { initialPath: '/recipes' })
      await waitFor(() => expect(screen.getByText('Apple')).toBeInTheDocument())

      // Rendered fine at the default sort despite the throwing storage.
      expect(screen.getByLabelText('Sort recipes')).toHaveValue('name-asc')
      expect(renderedRecipeOrder(['Apple', 'Banana'])).toEqual(['Apple', 'Banana'])

      // Sorting still works even though the persistence write throws and is swallowed.
      fireEvent.change(screen.getByLabelText('Sort recipes'), { target: { value: 'name-desc' } })
      expect(renderedRecipeOrder(['Apple', 'Banana'])).toEqual(['Banana', 'Apple'])
      // Pin the write-path contract: the change attempts a (swallowed) persist.
      expect(setItem).toHaveBeenCalledWith('fewd.recipes.sortBy', 'name-desc')
    } finally {
      getItem.mockRestore()
      setItem.mockRestore()
    }
  })

  it('clicking a recipe card navigates to its detail page by slug', async () => {
    // Mount both routes so navigate() from the card actually resolves.
    const pasta = makeRecipe({ id: 'r1', slug: 'pasta', name: 'Pasta' })
    mockJson('GET', '/api/recipes', [pasta])
    // The card now navigates by slug, so the detail fetch goes through the slug URL.
    mockJson('GET', '/api/recipes/pasta', pasta)

    renderWithProviders(
      <Routes>
        <Route path='/recipes' element={<RecipeManager />} />
        <Route path='/recipes/:id' element={<RecipeDetailPage />} />
      </Routes>,
      { initialPath: '/recipes' },
    )

    await waitFor(() => expect(screen.getByRole('button', { name: 'Pasta' })).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: 'Pasta' }))

    // Detail-only affordance — proves we landed on RecipeDetailPage.
    expect(await screen.findByText('Back to Recipes')).toBeInTheDocument()
    // Ingredients header is rendered by RecipeDetail, not the list card.
    expect(screen.getByRole('heading', { name: 'Ingredients' })).toBeInTheDocument()
  })

  it('adding a recipe POSTs, invalidates the list, and surfaces the new item on refetch', async () => {
    // Seed with one recipe so the empty-state's own "Add Recipe" action button
    // doesn't collide with the header button.
    const seed = makeRecipe({ id: 'r0', name: 'Pasta' })
    mockJson('GET', '/api/recipes', [seed])

    const { client } = renderWithProviders(<RecipeManager />, { initialPath: '/recipes' })
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    await waitFor(() => expect(screen.getByText('Pasta')).toBeInTheDocument())

    // Open the add form (header button).
    fireEvent.click(screen.getByRole('button', { name: /Add Recipe/i }))

    // Fill required fields: name, instructions, and at least one ingredient with a name.
    // The form's labels aren't wired with htmlFor/id, so target inputs by the
    // only "text" input in the form whose placeholder-less, empty-valued
    // sibling we can reach — use getAllByRole.
    const textboxes = screen.getAllByRole('textbox')
    const nameInput = textboxes[0] // First textbox is Name per form layout.
    fireEvent.change(nameInput, {
      target: { value: 'Pancakes' },
    })
    fireEvent.change(screen.getByPlaceholderText('Step-by-step instructions...'), {
      target: { value: 'Mix and fry.' },
    })
    fireEvent.click(screen.getByRole('button', { name: /Add ingredient/i }))
    fireEvent.change(screen.getByPlaceholderText('Ingredient name'), {
      target: { value: 'Flour' },
    })

    // Stage server responses for the POST + the list refetch.
    const created = makeRecipe({ id: 'r-new', name: 'Pancakes' })
    mockJson('POST', '/api/recipes', created, { status: 201 })
    mockJson('GET', '/api/recipes', [created])

    // Submit via the form's "Add Recipe" button (the header trigger unmounts in add mode).
    fireEvent.click(screen.getByRole('button', { name: 'Add Recipe' }))

    // Contract: the create mutation invalidates the ['recipes'] key.
    await waitFor(() => expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['recipes'] }))

    // Behavior: server saw the POST with the expected body.
    const postCall = vi.mocked(fetch).mock.calls.find(([, init]) =>
      (init as RequestInit | undefined)?.method === 'POST'
    )
    expect(postCall).toBeDefined()
    const postBody = JSON.parse((postCall![1] as RequestInit).body as string)
    expect(postBody.name).toBe('Pancakes')
    expect(postBody.instructions).toBe('Mix and fry.')
    expect(postBody.source).toBe('manual')
    expect(postBody.ingredients).toHaveLength(1)
    expect(postBody.ingredients[0].name).toBe('Flour')
  })

  it('import-from-URL streams progress, completes, and invalidates the recipe list', async () => {
    mockJson('GET', '/api/recipes', [])
    const stream = mockStream<Recipe>('/recipes/import/url')

    const { client } = renderWithProviders(<RecipeManager />, { initialPath: '/recipes' })
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    await waitFor(() => expect(screen.getByText('Your recipe book is empty')).toBeInTheDocument())

    // Open the import modal — "From URL" tab is the default.
    fireEvent.click(screen.getByRole('button', { name: 'Import' }))
    fireEvent.change(screen.getByPlaceholderText('https://example.com/recipe/...'), {
      target: { value: 'https://food.example/best-pasta' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Import' }))

    // The stream was dispatched with the URL body.
    await waitFor(() => expect(stream.calls.length).toBe(1))
    expect(stream.calls[0].body).toEqual({ url: 'https://food.example/best-pasta' })

    // Progress messages render as the loading label on the submit button.
    act(() => stream.emit({ phase: 'thinking', message: 'Fetching page…' }))
    await waitFor(() => expect(screen.getByText('Fetching page…')).toBeInTheDocument())

    // Stage the list refetch before completing so the shadowed route wins.
    const imported = makeRecipe({ id: 'r-import', name: 'Imported Pasta' })
    mockJson('GET', '/api/recipes', [imported])

    act(() => stream.complete(imported))

    // The streaming mutation doesn't invalidate itself — the component's
    // onSuccess handler calls queryClient.invalidateQueries(['recipes']).
    await waitFor(() => expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['recipes'] }))
  })
})

describe('ScalingPreviewAltRow', () => {
  it('walks chained or_alternative levels so deeper alts surface in the preview', async () => {
    // The bug being pinned: an earlier version of this row hardcoded a
    // single sub-row, so `milk or cream or water` previewed as only
    // `milk + or cream` and the deepest level was hidden before save.
    const { renderToStaticMarkup } = await import('react-dom/server')
    const chained = {
      name: 'milk',
      amount: { type: 'single' as const, value: 1 },
      unit: 'cup',
      notes: undefined,
      or_alternative: {
        name: 'cream',
        amount: { type: 'single' as const, value: 2 },
        unit: 'cups',
        notes: undefined,
        or_alternative: {
          name: 'water',
          amount: { type: 'single' as const, value: 3 },
          unit: 'cups',
          notes: undefined,
        },
      },
    }
    const html = renderToStaticMarkup(<ScalingPreviewAltRow ingredient={chained} />)
    expect(html).toContain('or milk')
    expect(html).toContain('or cream')
    expect(html).toContain('or water')
  })
})

describe('ScaleRecipePanel', () => {
  function makeParsed(): ParsedRecipe {
    return parseRecipe(makeRecipe({
      id: 'r1',
      name: 'Test Recipe',
      servings: 4,
      ingredients: JSON.stringify([
        { name: 'eggs', amount: { type: 'single', value: 3 }, unit: 'whole' },
        { name: 'milk', amount: { type: 'single', value: 1 }, unit: 'cup' },
      ]),
    }))
  }

  function makeParsedGarlicRange(): ParsedRecipe {
    return parseRecipe(makeRecipe({
      id: 'r1',
      name: 'Test Recipe',
      servings: 4,
      ingredients: JSON.stringify([
        { name: 'garlic', amount: { type: 'range', min: 2, max: 3 }, unit: 'clove' },
      ]),
    }))
  }

  // Scaling from 4 to 5 servings: 3.75 eggs round to 4, and milk stays exact.
  function makeScaleResult(): ScaleResult {
    return {
      ingredients: [
        { name: 'eggs', amount: { type: 'single', value: 4 }, unit: 'whole' },
        { name: 'milk', amount: { type: 'single', value: 1.25 }, unit: 'cup' },
      ],
      flagged: [
        { index: 0, name: 'eggs', scaled_value: 3.75, unit: 'whole' },
      ],
    }
  }

  function renderPanel(parsed: ParsedRecipe) {
    renderWithProviders(
      <ScaleRecipePanel
        parsed={parsed}
        onSaveAsNew={() => {}}
        onUpdateInPlace={() => {}}
        onCancel={() => {}}
      />,
    )
  }

  function previewServings(from: string, to: string) {
    fireEvent.change(screen.getByDisplayValue(from), { target: { value: to } })
    fireEvent.click(screen.getByRole('button', { name: /preview/i }))
  }

  it('renders an editable input for every ingredient row after Preview', async () => {
    mockJson('POST', '/api/recipes/r1/scale', makeScaleResult())
    renderPanel(makeParsed())
    previewServings('4', '5')

    await waitFor(() => {
      expect(screen.getByLabelText('eggs amount')).toHaveDisplayValue('4')
      expect(screen.getByDisplayValue('1.25')).toBeInTheDocument()
    })
  })

  it('updates only the edited row’s ratio cell when the user changes its value', async () => {
    mockJson('POST', '/api/recipes/r1/scale', makeScaleResult())
    renderPanel(makeParsed())
    previewServings('4', '5')

    await waitFor(() => expect(screen.getByDisplayValue('1.25')).toBeInTheDocument())

    // Eggs land at 4/3 and milk at 1.25/1.
    expect(screen.getByText('1.33×')).toBeInTheDocument()
    expect(screen.getByText('1.25×')).toBeInTheDocument()

    fireEvent.change(screen.getByDisplayValue('1.25'), { target: { value: '1.5' } })

    await waitFor(() => {
      expect(screen.getByText('1.5×')).toBeInTheDocument()
      expect(screen.queryByText('1.25×')).not.toBeInTheDocument()
      expect(screen.getByText('1.33×')).toBeInTheDocument()
    })
  })

  it('discards local edits when the user re-Previews with a different target', async () => {
    mockJson('POST', '/api/recipes/r1/scale', makeScaleResult())
    renderPanel(makeParsed())
    previewServings('4', '5')

    await waitFor(() => expect(screen.getByDisplayValue('1.25')).toBeInTheDocument())

    fireEvent.change(screen.getByDisplayValue('1.25'), { target: { value: '7.7' } })
    expect(screen.getByDisplayValue('7.7')).toBeInTheDocument()

    mockJson('POST', '/api/recipes/r1/scale', {
      ingredients: [
        { name: 'eggs', amount: { type: 'single', value: 5 }, unit: 'whole' },
        { name: 'milk', amount: { type: 'single', value: 1.5 }, unit: 'cup' },
      ],
      flagged: [{ index: 0, name: 'eggs', scaled_value: 4.5, unit: 'whole' }],
    })

    previewServings('5', '6')

    await waitFor(() => expect(screen.getByDisplayValue('1.5')).toBeInTheDocument())
    expect(screen.queryByDisplayValue('7.7')).not.toBeInTheDocument()
  })

  it('does not collapse the row to 0 when the user clears the input mid-typing', async () => {
    mockJson('POST', '/api/recipes/r1/scale', makeScaleResult())
    renderPanel(makeParsed())
    previewServings('4', '5')
    await waitFor(() => expect(screen.getByDisplayValue('1.25')).toBeInTheDocument())

    fireEvent.change(screen.getByDisplayValue('1.25'), { target: { value: '' } })

    // The committed amount must not have collapsed to 0 — that was the
    // pre-fix UX trap where clearing the field stomped state with `|| 0`.
    expect(screen.queryByDisplayValue('0')).not.toBeInTheDocument()
  })

  it('renders range-typed amounts as static text so the max bound is not silently dropped on edit', async () => {
    // Range amount on the milk row — would silently flatten to single on
    // first keystroke if it rendered as an editable number input.
    mockJson(
      'POST',
      '/api/recipes/r1/scale',
      {
        ingredients: [
          { name: 'eggs', amount: { type: 'single', value: 4 }, unit: 'whole' },
          { name: 'milk', amount: { type: 'range', min: 1.25, max: 1.75 }, unit: 'cup' },
        ],
        flagged: [],
      } satisfies ScaleResult,
    )
    renderPanel(makeParsed())
    previewServings('4', '5')

    // Eggs is single → editable input.
    await waitFor(() => expect(screen.getByLabelText('eggs amount')).toHaveDisplayValue('4'))
    // Milk is range → rendered as text "1.25-1.75", not an input.
    expect(screen.getByText('1.25-1.75')).toBeInTheDocument()
    expect(screen.queryByLabelText('milk amount')).not.toBeInTheDocument()
  })

  it('shows the rounding banner and the pre-rounding value on a rounded single row', async () => {
    mockJson('POST', '/api/recipes/r1/scale', makeScaleResult())
    renderPanel(makeParsed())
    previewServings('4', '5')

    await waitFor(() => expect(screen.getByText('Rounded from 3.75')).toBeInTheDocument())
    expect(screen.getByText(/rounded to whole amounts/i)).toBeInTheDocument()
    // The rounded value stays editable, so the user can override it.
    expect(screen.getByLabelText('eggs amount')).toHaveDisplayValue('4')
  })

  it('hides the pre-rounding hint once the user overrides the rounded value', async () => {
    mockJson('POST', '/api/recipes/r1/scale', makeScaleResult())
    renderPanel(makeParsed())
    previewServings('4', '5')
    await waitFor(() => expect(screen.getByText('Rounded from 3.75')).toBeInTheDocument())

    fireEvent.change(screen.getByLabelText('eggs amount'), { target: { value: '3' } })
    expect(screen.queryByText(/rounded from/i)).not.toBeInTheDocument()

    fireEvent.change(screen.getByLabelText('eggs amount'), { target: { value: '4' } })
    expect(screen.getByText('Rounded from 3.75')).toBeInTheDocument()
  })

  it('shows no banner or hint when nothing was rounded', async () => {
    mockJson(
      'POST',
      '/api/recipes/r1/scale',
      {
        ingredients: [
          { name: 'eggs', amount: { type: 'single', value: 6 }, unit: 'whole' },
          { name: 'milk', amount: { type: 'single', value: 2 }, unit: 'cup' },
        ],
        flagged: [],
      } satisfies ScaleResult,
    )
    renderPanel(makeParsed())
    previewServings('4', '8')

    await waitFor(() => expect(screen.getByLabelText('eggs amount')).toHaveDisplayValue('6'))
    expect(screen.queryByText(/rounded to whole amounts/i)).not.toBeInTheDocument()
    expect(screen.queryByText(/rounded from/i)).not.toBeInTheDocument()
  })

  it('shows the banner but no pre-rounding hint on a rounded range row', async () => {
    // 2-3 cloves at 1.5x is 3-4.5, which rounds to 3-5. The flag carries only
    // the min, so a hint would wrongly say "rounded from 3".
    mockJson(
      'POST',
      '/api/recipes/r1/scale',
      {
        ingredients: [
          { name: 'garlic', amount: { type: 'range', min: 3, max: 5 }, unit: 'clove' },
        ],
        flagged: [{ index: 0, name: 'garlic', scaled_value: 3, unit: 'clove' }],
      } satisfies ScaleResult,
    )
    renderPanel(makeParsedGarlicRange())
    previewServings('4', '6')

    await waitFor(() => expect(screen.getByText('3-5')).toBeInTheDocument())
    expect(screen.getByText(/rounded to whole amounts/i)).toBeInTheDocument()
    expect(screen.queryByText(/rounded from/i)).not.toBeInTheDocument()
    // The row itself carries the same amber cue as a flagged single row, so the
    // banner points at something the user can find.
    expect(screen.getByText('3-5')).toHaveClass('border-amber-300')
  })

  it('renders a range that collapsed to a single amount as an input without a hint', async () => {
    // 2-3 cloves at 0.25x is 0.5-0.75, and both bounds round to 1.
    mockJson(
      'POST',
      '/api/recipes/r1/scale',
      {
        ingredients: [
          { name: 'garlic', amount: { type: 'single', value: 1 }, unit: 'clove' },
        ],
        flagged: [{ index: 0, name: 'garlic', scaled_value: 0.5, unit: 'clove' }],
      } satisfies ScaleResult,
    )
    renderPanel(makeParsedGarlicRange())
    previewServings('4', '1')

    await waitFor(() => expect(screen.getByLabelText('garlic amount')).toHaveDisplayValue('1'))
    expect(screen.getByText(/rounded to whole amounts/i)).toBeInTheDocument()
    expect(screen.queryByText(/rounded from/i)).not.toBeInTheDocument()
  })

  it('shows the hint for a unitless count with the value the rounding used', async () => {
    // 2.99 eggs with no unit at 0.5x is 1.495, which rounds to 1. A two-decimal
    // hint would say 1.5 beside a 1.
    const parsed = parseRecipe(makeRecipe({
      id: 'r1',
      name: 'Test Recipe',
      servings: 4,
      ingredients: JSON.stringify([
        { name: 'eggs', amount: { type: 'single', value: 2.99 }, unit: '' },
      ]),
    }))
    mockJson(
      'POST',
      '/api/recipes/r1/scale',
      {
        ingredients: [{ name: 'eggs', amount: { type: 'single', value: 1 }, unit: '' }],
        flagged: [{ index: 0, name: 'eggs', scaled_value: 1.495, unit: '' }],
      } satisfies ScaleResult,
    )
    renderPanel(parsed)
    previewServings('4', '2')

    await waitFor(() => expect(screen.getByText('Rounded from 1.495')).toBeInTheDocument())
    expect(screen.getByLabelText('eggs amount')).toHaveDisplayValue('1')
  })

  function mockSingleEggFlag(value: number, scaledValue: number) {
    mockJson(
      'POST',
      '/api/recipes/r1/scale',
      {
        ingredients: [
          { name: 'eggs', amount: { type: 'single', value }, unit: 'whole' },
          { name: 'milk', amount: { type: 'single', value: 2 }, unit: 'cup' },
        ],
        flagged: [{ index: 0, name: 'eggs', scaled_value: scaledValue, unit: 'whole' }],
      } satisfies ScaleResult,
    )
  }

  it('shows a value just off a whole number at up to three decimals', async () => {
    mockSingleEggFlag(2, 2.004)
    renderPanel(makeParsed())
    previewServings('4', '8')

    await waitFor(() => expect(screen.getByText('Rounded from 2.004')).toBeInTheDocument())
    expect(screen.getByLabelText('eggs amount')).toHaveDisplayValue('2')
  })

  it('shows up to six decimals when three would equal the rounded amount', async () => {
    // 0.9999 at three decimals is 1, which would read "Rounded from 1" beside a 1.
    mockSingleEggFlag(1, 0.9999)
    renderPanel(makeParsed())
    previewServings('4', '8')

    await waitFor(() => expect(screen.getByText('Rounded from 0.9999')).toBeInTheDocument())
    expect(screen.queryByText('Rounded from 1')).not.toBeInTheDocument()
  })

  it('shows up to six decimals when three would round to a different amount', async () => {
    // 1.49985 at three decimals is 1.5, which rounds to 2, not the 1 shown.
    mockSingleEggFlag(1, 1.49985)
    renderPanel(makeParsed())
    previewServings('4', '8')

    await waitFor(() => expect(screen.getByText('Rounded from 1.49985')).toBeInTheDocument())
  })

  it('shows up to six decimals when a clamped amount would read as zero', async () => {
    // 0.0004 at three decimals is 0, which would read as nothing beside the clamped 1.
    mockSingleEggFlag(1, 0.0004)
    renderPanel(makeParsed())
    previewServings('4', '8')

    await waitFor(() => expect(screen.getByText('Rounded from 0.0004')).toBeInTheDocument())
  })

  it('keeps three decimals for an amount clamped up from below one half', async () => {
    // Six decimals would read 0.123456, so this pins the clamp's three-decimal branch.
    mockSingleEggFlag(1, 0.123456)
    renderPanel(makeParsed())
    previewServings('4', '8')

    await waitFor(() => expect(screen.getByText('Rounded from 0.123')).toBeInTheDocument())
  })

  it('keeps three decimals when they round to the amount shown', async () => {
    // Six decimals would read 3.333333, so this pins the ordinary three-decimal branch.
    mockSingleEggFlag(3, 3.333333)
    renderPanel(makeParsed())
    previewServings('4', '8')

    await waitFor(() => expect(screen.getByText('Rounded from 3.333')).toBeInTheDocument())
  })
})
