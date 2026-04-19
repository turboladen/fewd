import { fireEvent, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { makeMeal, makeMealTemplate, makePerson, makeRecipe } from '../test/factories'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { renderWithProviders } from '../test/renderWithProviders'
import type { PersonServing } from '../types/meal'
import { MealPlanner } from './MealPlanner'

// Pin "today" to a Monday so MealPlanner's week range is deterministic.
// Monday 2026-04-20 through Sunday 2026-04-26.
const MONDAY = '2026-04-20'
const SUNDAY = '2026-04-26'
const MEALS_URL = `/api/meals?start_date=${MONDAY}&end_date=${SUNDAY}`

beforeEach(() => {
  // toFake: ['Date'] — fake Date only, leave setTimeout/Promise etc. alone
  // so React Query's async flows still resolve naturally.
  vi.useFakeTimers({ toFake: ['Date'] })
  vi.setSystemTime(new Date('2026-04-20T12:00:00'))
  installFetchMock()
})
afterEach(() => {
  resetFetchMock()
  vi.useRealTimers()
})

describe('MealPlanner — plan a meal on a day slot', () => {
  it('creates a meal from an empty slot, closes the editor, and renders it back in the grid', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice' })
    const pasta = makeRecipe({ id: 'r-pasta', name: 'Pasta', servings: 4 })
    mockJson('GET', '/api/people', [alice])
    mockJson('GET', '/api/recipes', [pasta])
    mockJson('GET', MEALS_URL, [])
    mockJson('GET', '/api/meal-templates', [])

    const { client } = renderWithProviders(<MealPlanner />)
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    // Wait for the planner to render; 7 "Dinner" slot buttons (one per day).
    // Click Monday's (index 0).
    await waitFor(() => expect(screen.getAllByRole('button', { name: /Dinner/ }).length).toBe(7))
    const mondayDinner = screen.getAllByRole('button', { name: /Dinner/ })[0]
    expect(mondayDinner).toHaveTextContent('+ Plan')
    fireEvent.click(mondayDinner)

    // Editor opens with Alice + an "Add Recipe" button for her.
    await screen.findByRole('button', { name: 'Create Meal' })
    // PersonServingEditor renders an "+ Recipe" button per person; click Alice's.
    fireEvent.click(screen.getByRole('button', { name: /Recipe/ }))

    // Select Pasta in the first recipe dropdown (only combobox on the screen).
    const recipeSelect = await screen.findByRole('combobox')
    fireEvent.change(recipeSelect, { target: { value: 'r-pasta' } })

    // Stage the POST response and the post-invalidation GET refetch.
    const pastaServing: PersonServing = {
      food_type: 'recipe',
      person_id: 'p1',
      recipe_id: 'r-pasta',
      servings_count: 1,
      notes: null,
    }
    const createdMeal = makeMeal({
      id: 'm-new',
      date: MONDAY,
      meal_type: 'Dinner',
      order_index: 2,
      servings: [pastaServing],
    })
    mockJson('POST', '/api/meals', createdMeal)
    mockJson('GET', MEALS_URL, [createdMeal]) // shadow: post-mutation refetch

    fireEvent.click(screen.getByRole('button', { name: 'Create Meal' }))

    // After save: editor closes and the slot now shows Alice + Pasta.
    await waitFor(() => {
      expect(screen.queryByRole('button', { name: 'Create Meal' })).not.toBeInTheDocument()
    })
    // The Monday Dinner slot rendering the meal now includes Alice and Pasta.
    await waitFor(() => expect(screen.getAllByText(/Pasta/).length).toBeGreaterThan(0))
    expect(screen.getAllByText(/Alice/).length).toBeGreaterThan(0)

    // Invalidation contract: useCreateMeal invalidates both 'meals' and 'recipes'.
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['meals'] })
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['recipes'] })
  })
})

describe('MealPlanner — applying a template', () => {
  it('merges a template\'s servings into the editor when "Use Template" is clicked', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice' })
    const pizza = makeRecipe({ id: 'r-pizza', name: 'Pizza', servings: 4 })
    const pizzaServing: PersonServing = {
      food_type: 'recipe',
      person_id: 'p1',
      recipe_id: 'r-pizza',
      servings_count: 2,
      notes: null,
    }
    const template = makeMealTemplate({
      id: 't-pizza',
      name: 'Pizza Friday',
      meal_type: 'Dinner',
      servings: [pizzaServing],
    })
    mockJson('GET', '/api/people', [alice])
    mockJson('GET', '/api/recipes', [pizza])
    mockJson('GET', MEALS_URL, [])
    mockJson('GET', '/api/meal-templates', [template])

    renderWithProviders(<MealPlanner />)

    // Open the Monday Dinner editor (first of 7 Dinner slots).
    await waitFor(() => expect(screen.getAllByRole('button', { name: /Dinner/ }).length).toBe(7))
    fireEvent.click(screen.getAllByRole('button', { name: /Dinner/ })[0])
    await screen.findByRole('button', { name: 'Create Meal' })

    // Click "Use Template" — picker expands with the template row.
    fireEvent.click(screen.getByRole('button', { name: 'Use Template' }))
    const templateRow = await screen.findByRole('button', { name: /Pizza Friday/ })

    fireEvent.click(templateRow)

    // The editor now has a serving for Alice with Pizza selected.
    // Verify the recipe dropdown has pizza selected (value = r-pizza).
    await waitFor(() => {
      const selects = screen.getAllByRole('combobox') as HTMLSelectElement[]
      const pizzaSelected = selects.some((s) => s.value === 'r-pizza')
      expect(pizzaSelected).toBe(true)
    })
  })
})

describe('MealPlanner — ServingMismatchBanner wiring', () => {
  it('shows the serving-mismatch banner inside the editor when totals < recipe servings', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice' })
    const bob = makePerson({ id: 'p2', name: 'Bob' })
    // Recipe makes 4 servings.
    const pasta = makeRecipe({ id: 'r-pasta', name: 'Pasta', servings: 4 })
    // Existing meal: Alice + Bob each take 1 serving (total planned = 2, recipe makes 4 → mismatch).
    const existingMeal = makeMeal({
      id: 'm-existing',
      date: MONDAY,
      meal_type: 'Dinner',
      order_index: 2,
      servings: [
        {
          food_type: 'recipe',
          person_id: 'p1',
          recipe_id: 'r-pasta',
          servings_count: 1,
          notes: null,
        },
        {
          food_type: 'recipe',
          person_id: 'p2',
          recipe_id: 'r-pasta',
          servings_count: 1,
          notes: null,
        },
      ],
    })
    mockJson('GET', '/api/people', [alice, bob])
    mockJson('GET', '/api/recipes', [pasta])
    mockJson('GET', MEALS_URL, [existingMeal])
    mockJson('GET', '/api/meal-templates', [])

    renderWithProviders(<MealPlanner />)

    // Wait for the meal to render in the grid (shows Pasta text in the slot).
    await waitFor(() => expect(screen.getAllByText(/Pasta/).length).toBeGreaterThan(0))

    // Open the Monday Dinner editor (click the slot containing the existing meal).
    const dinnerSlots = screen.getAllByRole('button', { name: /Dinner/ })
    fireEvent.click(dinnerSlots[0])

    // ServingMismatchBanner copy: "Pasta makes 4 servings, but you've planned 2."
    await waitFor(() => {
      expect(screen.getByText(/makes 4/)).toBeInTheDocument()
    })
    expect(screen.getByRole('button', { name: /Adjust to Full Recipe/ })).toBeInTheDocument()
  })
})

describe('MealPlanner — delete a planned meal', () => {
  it('removes a meal via DELETE and the slot reverts to "+ Plan"', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice' })
    const pasta = makeRecipe({ id: 'r-pasta', name: 'Pasta', servings: 4 })
    const existingMeal = makeMeal({
      id: 'm-existing',
      date: MONDAY,
      meal_type: 'Dinner',
      order_index: 2,
      servings: [{
        food_type: 'recipe',
        person_id: 'p1',
        recipe_id: 'r-pasta',
        servings_count: 4,
        notes: null,
      }],
    })
    mockJson('GET', '/api/people', [alice])
    mockJson('GET', '/api/recipes', [pasta])
    mockJson('GET', MEALS_URL, [existingMeal])
    mockJson('GET', '/api/meal-templates', [])

    renderWithProviders(<MealPlanner />)

    // Wait for the meal to render.
    await waitFor(() => expect(screen.getAllByText(/Pasta/).length).toBeGreaterThan(0))

    // Open the Monday Dinner editor.
    const dinnerSlots = screen.getAllByRole('button', { name: /Dinner/ })
    fireEvent.click(dinnerSlots[0])
    await screen.findByRole('button', { name: 'Save Changes' })

    // Stage the DELETE + shadowed refetch (empty list).
    mockJson('DELETE', '/api/meals/m-existing', null, { status: 204 })
    mockJson('GET', MEALS_URL, [])

    // Two-step delete: click Delete, then confirm Yes.
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }))
    fireEvent.click(screen.getByRole('button', { name: 'Yes' }))

    // Editor closes and Pasta disappears from the grid.
    await waitFor(() => {
      expect(screen.queryByText(/Pasta/)).not.toBeInTheDocument()
    })
  })
})
