import { act, fireEvent, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { renderWithProviders } from '../test/renderWithProviders'
import { installStreamMock, mockStream, resetStreamMock } from '../test/streamMock'
import type { BarItem } from '../types/barItem'
import type { AiSuggestCocktailsDto, CreateDrinkRecipeDto, DrinkRecipe } from '../types/drinkRecipe'
import type { Person } from '../types/person'
import { CocktailSuggester } from './CocktailSuggester'

function makePerson(overrides: Partial<Person> = {}): Person {
  return {
    id: 'p1',
    name: 'Alice',
    // Adult (>= 21) by default so the suggester selects them automatically.
    birthdate: '1990-01-01',
    dietary_goals: null,
    dislikes: '[]',
    favorites: '[]',
    notes: null,
    drink_preferences: null,
    drink_dislikes: null,
    is_active: true,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

function makeBarItem(overrides: Partial<BarItem> = {}): BarItem {
  return {
    id: 'bi-1',
    name: 'Bourbon',
    category: 'spirit',
    created_at: '2026-04-01T00:00:00Z',
    ...overrides,
  }
}

function makeDrinkRecipe(overrides: Partial<DrinkRecipe> = {}): DrinkRecipe {
  return {
    id: 'd1',
    slug: 'old-fashioned',
    name: 'Old Fashioned',
    description: 'Classic ancestral cocktail',
    source: 'manual',
    source_url: null,
    servings: 1,
    instructions: 'Stir with ice, strain.',
    ingredients: JSON.stringify([
      { name: 'Bourbon', amount: { type: 'single', value: 2 }, unit: 'oz' },
    ]),
    technique: 'stirred',
    glassware: 'rocks',
    garnish: 'orange peel',
    tags: JSON.stringify(['old fashioned', 'ancestral']),
    notes: null,
    icon: '🥃',
    is_favorite: false,
    is_non_alcoholic: false,
    rating: null,
    times_made: 0,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
    ...overrides,
  }
}

function seedBaseline(
  people: Person[],
  barItems: BarItem[],
  drinkRecipes: DrinkRecipe[],
) {
  mockJson('GET', '/api/people', people)
  mockJson('GET', '/api/bar-items', barItems)
  mockJson('GET', '/api/drink-recipes', drinkRecipes)
  mockJson('GET', '/api/settings/anthropic_api_key', 'sk-ant-test-key')
  mockJson('GET', '/api/settings/claude_model', 'claude-sonnet-4-20250514')
  mockJson('GET', '/api/settings/models', [])
}

beforeEach(() => {
  installFetchMock()
  installStreamMock()
})
afterEach(() => {
  resetFetchMock()
  resetStreamMock()
})

describe('CocktailSuggester', () => {
  it('configure → generate → streaming progress → results: AI suggestion renders after complete()', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice' })
    const bourbon = makeBarItem({ id: 'bi-1', name: 'Bourbon', category: 'spirit' })
    const bitters = makeBarItem({ id: 'bi-2', name: 'Angostura Bitters', category: 'bitter' })
    seedBaseline([alice], [bourbon, bitters], [])
    const stream = mockStream<CreateDrinkRecipeDto[]>('/cocktails/suggest')

    renderWithProviders(<CocktailSuggester />)

    // Wait for seeds to load — adults + all bar items auto-select.
    await waitFor(() => expect(screen.getByText('Alice')).toBeInTheDocument())
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Suggest Drinks' })).not.toBeDisabled()
    )

    fireEvent.click(screen.getByRole('button', { name: 'Suggest Drinks' }))

    // The stream should have been triggered with the expected DTO shape.
    await waitFor(() => expect(stream.calls.length).toBe(1))
    const body = stream.calls[0].body as AiSuggestCocktailsDto
    expect(body.person_ids).toEqual(['p1'])
    expect(new Set(body.bar_item_ids)).toEqual(new Set(['bi-1', 'bi-2']))
    expect(body.mood).toEqual({ type: 'style', label: 'Ancestrals' })
    expect(body.include_non_alcoholic).toBe(false)

    // Progress event renders in the results phase.
    act(() => {
      stream.emit({ phase: 'thinking', message: 'Brewing ideas…' })
    })
    expect(screen.getByText('Brewing ideas…')).toBeInTheDocument()

    // Complete the stream with a single suggestion.
    const suggestion: CreateDrinkRecipeDto = {
      name: 'House Sazerac',
      source: 'ai-suggestion',
      servings: 1,
      instructions: 'Stir, strain.',
      ingredients: [
        { name: 'Rye', amount: { type: 'single', value: 2 }, unit: 'oz' },
      ],
      tags: ['ancestral'],
      description: 'A bittered rye riff',
      icon: '🥃',
    }
    act(() => {
      stream.complete([suggestion])
    })

    await waitFor(() => expect(screen.getByText('House Sazerac')).toBeInTheDocument())
    expect(screen.getByText('A bittered rye riff')).toBeInTheDocument()
  })

  it('accepting a suggestion POSTs to drink-recipes and invalidates the list', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice' })
    seedBaseline([alice], [makeBarItem({ id: 'bi-1', name: 'Bourbon', category: 'spirit' })], [])
    const stream = mockStream<CreateDrinkRecipeDto[]>('/cocktails/suggest')

    const { client } = renderWithProviders(<CocktailSuggester />)
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Suggest Drinks' })).not.toBeDisabled()
    )
    fireEvent.click(screen.getByRole('button', { name: 'Suggest Drinks' }))

    const suggestion: CreateDrinkRecipeDto = {
      name: 'House Sazerac',
      source: 'ai-suggestion',
      servings: 1,
      instructions: 'Stir, strain.',
      ingredients: [],
      tags: [],
    }
    await waitFor(() => expect(stream.calls.length).toBe(1))
    act(() => stream.complete([suggestion]))
    await waitFor(() => expect(screen.getByText('House Sazerac')).toBeInTheDocument())

    // Expand the card to reveal the Save button, then save.
    fireEvent.click(screen.getByText('House Sazerac'))
    const saved = makeDrinkRecipe({ id: 'd-new', name: 'House Sazerac' })
    mockJson('POST', '/api/drink-recipes', saved, { status: 201 })
    // Silence refetch of list with a matching payload.
    mockJson('GET', '/api/drink-recipes', [saved])

    fireEvent.click(screen.getByRole('button', { name: 'Save This Drink' }))

    await waitFor(() => expect(screen.getByRole('button', { name: 'Saved' })).toBeInTheDocument())
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['drink-recipes'] })
  })

  it('streaming error surfaces in the results phase and leaves Generate usable', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice' })
    seedBaseline([alice], [makeBarItem({ id: 'bi-1', name: 'Bourbon', category: 'spirit' })], [])
    const stream = mockStream<CreateDrinkRecipeDto[]>('/cocktails/suggest')

    renderWithProviders(<CocktailSuggester />)

    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Suggest Drinks' })).not.toBeDisabled()
    )
    fireEvent.click(screen.getByRole('button', { name: 'Suggest Drinks' }))

    await waitFor(() => expect(stream.calls.length).toBe(1))
    act(() => stream.error(new Error('AI endpoint down')))

    expect(await screen.findByText('AI endpoint down')).toBeInTheDocument()
    // Regenerate is re-enabled (not stuck in "Generating…").
    expect(screen.getByRole('button', { name: /Regenerate/ })).not.toBeDisabled()
  })

  it('recipes-only source skips the stream and matches saved recipes against the bar', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice' })
    const bourbon = makeBarItem({ id: 'bi-1', name: 'Bourbon', category: 'spirit' })
    const oldFashioned = makeDrinkRecipe({ id: 'd1', name: 'Old Fashioned' })
    seedBaseline([alice], [bourbon], [oldFashioned])

    // Register the stream path but expect it NEVER to fire.
    const stream = mockStream<CreateDrinkRecipeDto[]>('/cocktails/suggest')

    renderWithProviders(<CocktailSuggester />)

    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Suggest Drinks' })).not.toBeDisabled()
    )

    // Pick the "Recipes Only" source.
    fireEvent.click(screen.getByRole('button', { name: 'Recipes Only' }))

    // Button text flips to the recipes-only phrasing.
    const findBtn = screen.getByRole('button', { name: 'Find Matching Recipes' })
    fireEvent.click(findBtn)

    // Recipe match renders in the "From Your Recipe Book" section.
    await waitFor(() => expect(screen.getByText('Old Fashioned')).toBeInTheDocument())
    expect(screen.getByText('Classic ancestral cocktail')).toBeInTheDocument()
    // Stream was never triggered.
    expect(stream.calls.length).toBe(0)
  })
})
