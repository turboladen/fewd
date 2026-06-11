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

describe('MealPlanner — default recipe for a new serving', () => {
  // servings_count: 4 matches the recipes' full yield so no ServingMismatchBanner
  // renders — its "Adjust to Full Recipe" button would match /Recipe/ queries below.
  function makeRecipeServing(personId: string, recipeId: string): PersonServing {
    return {
      food_type: 'recipe',
      person_id: personId,
      recipe_id: recipeId,
      servings_count: 4,
      notes: null,
    }
  }

  it('defaults a new serving to the recipe another person already picked in the slot', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice' })
    const bob = makePerson({ id: 'p2', name: 'Bob' })
    // 'Bacon Fried Rice' sorts first alphabetically — the old buggy default.
    const baconFriedRice = makeRecipe({ id: 'r-bacon', name: 'Bacon Fried Rice', servings: 4 })
    const tacos = makeRecipe({ id: 'r-tacos', name: 'Tacos', servings: 4 })
    const existingMeal = makeMeal({
      id: 'm-existing',
      date: MONDAY,
      meal_type: 'Dinner',
      order_index: 2,
      servings: [makeRecipeServing('p1', 'r-tacos')],
    })
    mockJson('GET', '/api/people', [alice, bob])
    mockJson('GET', '/api/recipes', [baconFriedRice, tacos])
    mockJson('GET', MEALS_URL, [existingMeal])
    mockJson('GET', '/api/meal-templates', [])

    renderWithProviders(<MealPlanner />)

    // Open the Monday Dinner editor.
    await waitFor(() => expect(screen.getAllByText(/Tacos/).length).toBeGreaterThan(0))
    fireEvent.click(screen.getAllByRole('button', { name: /Dinner/ })[0])
    await screen.findByRole('button', { name: 'Save Changes' })

    // Click Bob's "+ Recipe" button (one per person; Bob is second).
    const addRecipeButtons = screen.getAllByRole('button', { name: /Recipe/ })
    fireEvent.click(addRecipeButtons[1])

    // Bob's new serving defaults to Tacos — the recipe Alice already picked.
    await waitFor(() => {
      const selects = screen.getAllByRole('combobox') as HTMLSelectElement[]
      expect(selects).toHaveLength(2)
      expect(selects[1].value).toBe('r-tacos')
    })
  })

  it('defaults to the most-recently-added recipe when the slot has several', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice' })
    const bob = makePerson({ id: 'p2', name: 'Bob' })
    const pasta = makeRecipe({ id: 'r-pasta', name: 'Pasta', servings: 4 })
    const tacos = makeRecipe({ id: 'r-tacos', name: 'Tacos', servings: 4 })
    // Alice picked Pasta first, then Tacos — Tacos is the latest pick.
    const existingMeal = makeMeal({
      id: 'm-existing',
      date: MONDAY,
      meal_type: 'Dinner',
      order_index: 2,
      servings: [makeRecipeServing('p1', 'r-pasta'), makeRecipeServing('p1', 'r-tacos')],
    })
    mockJson('GET', '/api/people', [alice, bob])
    mockJson('GET', '/api/recipes', [pasta, tacos])
    mockJson('GET', MEALS_URL, [existingMeal])
    mockJson('GET', '/api/meal-templates', [])

    renderWithProviders(<MealPlanner />)

    await waitFor(() => expect(screen.getAllByText(/Pasta/).length).toBeGreaterThan(0))
    fireEvent.click(screen.getAllByRole('button', { name: /Dinner/ })[0])
    await screen.findByRole('button', { name: 'Save Changes' })

    const addRecipeButtons = screen.getAllByRole('button', { name: /Recipe/ })
    fireEvent.click(addRecipeButtons[1])

    await waitFor(() => {
      const selects = screen.getAllByRole('combobox') as HTMLSelectElement[]
      expect(selects).toHaveLength(3)
      expect(selects[2].value).toBe('r-tacos')
    })
  })

  it('follows the latest explicit pick, not map order, when an earlier person re-picks', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice' })
    const bob = makePerson({ id: 'p2', name: 'Bob' })
    const carol = makePerson({ id: 'p3', name: 'Carol' })
    // servings: 1 so a 1-serving pick is a full recipe — no ServingMismatchBanner
    // whose "Adjust to Full Recipe" button would match the /Recipe/ queries below.
    const pasta = makeRecipe({ id: 'r-pasta', name: 'Pasta', servings: 1 })
    const tacos = makeRecipe({ id: 'r-tacos', name: 'Tacos', servings: 1 })
    mockJson('GET', '/api/people', [alice, bob, carol])
    mockJson('GET', '/api/recipes', [pasta, tacos])
    mockJson('GET', MEALS_URL, [])
    mockJson('GET', '/api/meal-templates', [])

    renderWithProviders(<MealPlanner />)

    await waitFor(() => expect(screen.getAllByRole('button', { name: /Dinner/ }).length).toBe(7))
    fireEvent.click(screen.getAllByRole('button', { name: /Dinner/ })[0])
    await screen.findByRole('button', { name: 'Create Meal' })

    // Alice picks Pasta.
    fireEvent.click(screen.getAllByRole('button', { name: /Recipe/ })[0])
    const aliceSelect = await screen.findByRole('combobox') as HTMLSelectElement
    fireEvent.change(aliceSelect, { target: { value: 'r-pasta' } })

    // Bob's new serving defaults to Alice's pick.
    fireEvent.click(screen.getAllByRole('button', { name: /Recipe/ })[1])
    await waitFor(() => {
      const selects = screen.getAllByRole('combobox') as HTMLSelectElement[]
      expect(selects).toHaveLength(2)
      expect(selects[1].value).toBe('r-pasta')
    })

    // Alice changes her mind to Tacos — now the latest explicit pick,
    // even though Bob's Pasta serving comes later in map order.
    fireEvent.change(
      (screen.getAllByRole('combobox') as HTMLSelectElement[])[0],
      { target: { value: 'r-tacos' } },
    )

    // Carol's new serving follows Alice's re-pick, not Bob's older Pasta.
    fireEvent.click(screen.getAllByRole('button', { name: /Recipe/ })[2])
    await waitFor(() => {
      const selects = screen.getAllByRole('combobox') as HTMLSelectElement[]
      expect(selects).toHaveLength(3)
      expect(selects[2].value).toBe('r-tacos')
    })
  })

  it('defaults the first serving in an empty slot to unselected, not the first recipe', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice' })
    const baconFriedRice = makeRecipe({ id: 'r-bacon', name: 'Bacon Fried Rice', servings: 4 })
    mockJson('GET', '/api/people', [alice])
    mockJson('GET', '/api/recipes', [baconFriedRice])
    mockJson('GET', MEALS_URL, [])
    mockJson('GET', '/api/meal-templates', [])

    renderWithProviders(<MealPlanner />)

    await waitFor(() => expect(screen.getAllByRole('button', { name: /Dinner/ }).length).toBe(7))
    fireEvent.click(screen.getAllByRole('button', { name: /Dinner/ })[0])
    await screen.findByRole('button', { name: 'Create Meal' })

    fireEvent.click(screen.getByRole('button', { name: /Recipe/ }))

    const recipeSelect = await screen.findByRole('combobox') as HTMLSelectElement
    expect(recipeSelect.value).toBe('')
  })

  it('blocks saving a serving with no recipe selected', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice' })
    const pasta = makeRecipe({ id: 'r-pasta', name: 'Pasta', servings: 4 })
    mockJson('GET', '/api/people', [alice])
    mockJson('GET', '/api/recipes', [pasta])
    mockJson('GET', MEALS_URL, [])
    mockJson('GET', '/api/meal-templates', [])
    // No POST /api/meals mock registered — a save attempt would throw.

    renderWithProviders(<MealPlanner />)

    await waitFor(() => expect(screen.getAllByRole('button', { name: /Dinner/ }).length).toBe(7))
    fireEvent.click(screen.getAllByRole('button', { name: /Dinner/ })[0])
    await screen.findByRole('button', { name: 'Create Meal' })

    // Add a recipe serving but leave it unselected.
    fireEvent.click(screen.getByRole('button', { name: /Recipe/ }))
    await screen.findByRole('combobox')

    fireEvent.click(screen.getByRole('button', { name: 'Create Meal' }))

    // Validation error appears and the editor stays open.
    await screen.findByText(/select a recipe/i)
    expect(screen.getByRole('button', { name: 'Create Meal' })).toBeInTheDocument()
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
