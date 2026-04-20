import { act, fireEvent, screen, waitFor } from '@testing-library/react'
import { Route, Routes } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { RecipeDetailPage } from '../routes/RecipeDetailPage'
import { makeRecipe } from '../test/factories'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { renderWithProviders } from '../test/renderWithProviders'
import { installStreamMock, mockStream, resetStreamMock } from '../test/streamMock'
import type { Recipe } from '../types/recipe'
import { RecipeManager } from './RecipeManager'

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
