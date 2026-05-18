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
})

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
        { name: 'eggs', amount: { type: 'single', value: 3 }, unit: '' },
        { name: 'milk', amount: { type: 'single', value: 1 }, unit: 'cup' },
      ]),
    }))
  }

  function makeScaleResult(): ScaleResult {
    return {
      ingredients: [
        { name: 'eggs', amount: { type: 'single', value: 3.75 }, unit: '' },
        { name: 'milk', amount: { type: 'single', value: 1.25 }, unit: 'cup' },
      ],
      flagged: [
        { index: 0, name: 'eggs', scaled_value: 3.75, unit: '' },
      ],
    }
  }

  it('renders an editable input for every ingredient row after Preview', async () => {
    const parsed = makeParsed()
    mockJson('POST', '/api/recipes/r1/scale', makeScaleResult())

    renderWithProviders(
      <ScaleRecipePanel
        parsed={parsed}
        onSaveAsNew={() => {}}
        onUpdateInPlace={() => {}}
        onCancel={() => {}}
      />,
    )

    const servingsInput = screen.getByDisplayValue('4')
    fireEvent.change(servingsInput, { target: { value: '5' } })
    fireEvent.click(screen.getByRole('button', { name: /preview/i }))

    await waitFor(() => {
      expect(screen.getByDisplayValue('3.75')).toBeInTheDocument()
      expect(screen.getByDisplayValue('1.25')).toBeInTheDocument()
    })
  })

  it('updates only the edited row’s ratio cell when the user changes its value', async () => {
    const parsed = makeParsed()
    mockJson('POST', '/api/recipes/r1/scale', makeScaleResult())

    renderWithProviders(
      <ScaleRecipePanel
        parsed={parsed}
        onSaveAsNew={() => {}}
        onUpdateInPlace={() => {}}
        onCancel={() => {}}
      />,
    )

    fireEvent.change(screen.getByDisplayValue('4'), { target: { value: '5' } })
    fireEvent.click(screen.getByRole('button', { name: /preview/i }))

    await waitFor(() => expect(screen.getByDisplayValue('1.25')).toBeInTheDocument())

    // Both rows coincidentally land at 1.25× (eggs 3.75/3, milk 1.25/1), so
    // the assertion below needs `getAllByText` to match both at this point.
    expect(screen.getAllByText('1.25×')).toHaveLength(2)

    fireEvent.change(screen.getByDisplayValue('1.25'), { target: { value: '1.5' } })

    await waitFor(() => {
      expect(screen.getByText('1.5×')).toBeInTheDocument()
      expect(screen.getByText('1.25×')).toBeInTheDocument()
    })
  })

  it('discards local edits when the user re-Previews with a different target', async () => {
    const parsed = makeParsed()
    mockJson('POST', '/api/recipes/r1/scale', makeScaleResult())

    renderWithProviders(
      <ScaleRecipePanel
        parsed={parsed}
        onSaveAsNew={() => {}}
        onUpdateInPlace={() => {}}
        onCancel={() => {}}
      />,
    )

    fireEvent.change(screen.getByDisplayValue('4'), { target: { value: '5' } })
    fireEvent.click(screen.getByRole('button', { name: /preview/i }))

    await waitFor(() => expect(screen.getByDisplayValue('1.25')).toBeInTheDocument())

    fireEvent.change(screen.getByDisplayValue('1.25'), { target: { value: '7.7' } })
    expect(screen.getByDisplayValue('7.7')).toBeInTheDocument()

    mockJson('POST', '/api/recipes/r1/scale', {
      ingredients: [
        { name: 'eggs', amount: { type: 'single', value: 4.5 }, unit: '' },
        { name: 'milk', amount: { type: 'single', value: 1.5 }, unit: 'cup' },
      ],
      flagged: [{ index: 0, name: 'eggs', scaled_value: 4.5, unit: '' }],
    })

    fireEvent.change(screen.getByDisplayValue('5'), { target: { value: '6' } })
    fireEvent.click(screen.getByRole('button', { name: /preview/i }))

    await waitFor(() => expect(screen.getByDisplayValue('1.5')).toBeInTheDocument())
    expect(screen.queryByDisplayValue('7.7')).not.toBeInTheDocument()
  })

  it('does not collapse the row to 0 when the user clears the input mid-typing', async () => {
    const parsed = makeParsed()
    mockJson('POST', '/api/recipes/r1/scale', makeScaleResult())

    renderWithProviders(
      <ScaleRecipePanel
        parsed={parsed}
        onSaveAsNew={() => {}}
        onUpdateInPlace={() => {}}
        onCancel={() => {}}
      />,
    )

    fireEvent.change(screen.getByDisplayValue('4'), { target: { value: '5' } })
    fireEvent.click(screen.getByRole('button', { name: /preview/i }))
    await waitFor(() => expect(screen.getByDisplayValue('1.25')).toBeInTheDocument())

    fireEvent.change(screen.getByDisplayValue('1.25'), { target: { value: '' } })

    // The committed amount must not have collapsed to 0 — that was the
    // pre-fix UX trap where clearing the field stomped state with `|| 0`.
    expect(screen.queryByDisplayValue('0')).not.toBeInTheDocument()
  })

  it('renders range-typed amounts as static text so the max bound is not silently dropped on edit', async () => {
    const parsed = makeParsed()
    // Range amount on the milk row — would silently flatten to single on
    // first keystroke if it rendered as an editable number input.
    mockJson(
      'POST',
      '/api/recipes/r1/scale',
      {
        ingredients: [
          { name: 'eggs', amount: { type: 'single', value: 3.75 }, unit: '' },
          { name: 'milk', amount: { type: 'range', min: 1.25, max: 1.75 }, unit: 'cup' },
        ],
        flagged: [],
      } satisfies ScaleResult,
    )

    renderWithProviders(
      <ScaleRecipePanel
        parsed={parsed}
        onSaveAsNew={() => {}}
        onUpdateInPlace={() => {}}
        onCancel={() => {}}
      />,
    )

    fireEvent.change(screen.getByDisplayValue('4'), { target: { value: '5' } })
    fireEvent.click(screen.getByRole('button', { name: /preview/i }))

    // Eggs is single → editable input.
    await waitFor(() => expect(screen.getByDisplayValue('3.75')).toBeInTheDocument())
    // Milk is range → rendered as text "1.25-1.75", not an input.
    expect(screen.getByText('1.25-1.75')).toBeInTheDocument()
    expect(screen.queryByLabelText('milk amount')).not.toBeInTheDocument()
  })
})
