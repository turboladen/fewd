import { describe, expect, it } from 'vitest'
import type { Ingredient, IngredientAmount, Recipe, TimeValue } from './recipe'
import {
  formatAmount,
  formatIngredientLabel,
  formatTime,
  parseInstructionSteps,
  parseRecipe,
} from './recipe'

describe('formatTime', () => {
  it('formats time value', () => {
    expect(formatTime({ value: 15, unit: 'minutes' })).toBe('15 minutes')
    expect(formatTime({ value: 2, unit: 'hours' })).toBe('2 hours')
  })

  it('returns empty string for null', () => {
    expect(formatTime(null)).toBe('')
  })
})

describe('formatIngredientLabel', () => {
  const base = (overrides: Partial<Ingredient>): Ingredient => ({
    name: 'garlic',
    amount: { type: 'single', value: 1 },
    unit: 'clove',
    ...overrides,
  })

  it('returns name alone when prep is missing', () => {
    expect(formatIngredientLabel(base({}))).toBe('garlic')
  })

  it('appends prep with comma when present', () => {
    expect(formatIngredientLabel(base({ prep: 'minced' }))).toBe('garlic, minced')
  })

  it('treats empty-string prep as absent', () => {
    expect(formatIngredientLabel(base({ prep: '' }))).toBe('garlic')
  })
})

describe('formatAmount', () => {
  it('formats integer single amount', () => {
    const amount: IngredientAmount = { type: 'single', value: 3 }
    expect(formatAmount(amount)).toBe('3')
  })

  it('formats decimal single amount without trailing zeros', () => {
    const amount: IngredientAmount = { type: 'single', value: 1.5 }
    expect(formatAmount(amount)).toBe('1.5')
  })

  it('formats very precise decimal', () => {
    const amount: IngredientAmount = { type: 'single', value: 0.25 }
    expect(formatAmount(amount)).toBe('0.25')
  })

  it('formats range amount', () => {
    const amount: IngredientAmount = { type: 'range', min: 1, max: 2 }
    expect(formatAmount(amount)).toBe('1-2')
  })
})

describe('parseRecipe', () => {
  const makeRecipe = (overrides: Partial<Recipe> = {}): Recipe => ({
    id: 'test-id',
    slug: 'test-recipe',
    name: 'Test Recipe',
    description: null,
    source: 'manual',
    parent_recipe_id: null,
    prep_time: null,
    cook_time: null,
    total_time: null,
    servings: 4,
    portion_size: null,
    instructions: 'Mix and cook',
    ingredients: JSON.stringify([
      { name: 'flour', amount: { type: 'single', value: 2 }, unit: 'cups' },
    ]),
    nutrition_per_serving: null,
    tags: JSON.stringify(['dinner']),
    notes: null,
    icon: null,
    is_favorite: false,
    times_made: 0,
    last_made: null,
    rating: null,
    created_at: '2025-01-01T00:00:00Z',
    updated_at: '2025-01-01T00:00:00Z',
    ...overrides,
  })

  it('parses JSON string fields', () => {
    const parsed = parseRecipe(makeRecipe())
    expect(parsed.ingredients).toHaveLength(1)
    expect(parsed.ingredients[0].name).toBe('flour')
    expect(parsed.tags).toEqual(['dinner'])
  })

  it('parses time values from JSON', () => {
    const time: TimeValue = { value: 15, unit: 'minutes' }
    const parsed = parseRecipe(makeRecipe({
      prep_time: JSON.stringify(time),
    }))
    expect(parsed.prep_time).toEqual(time)
  })

  it('handles null optional fields', () => {
    const parsed = parseRecipe(makeRecipe())
    expect(parsed.prep_time).toBeNull()
    expect(parsed.cook_time).toBeNull()
    expect(parsed.nutrition_per_serving).toBeNull()
    expect(parsed.portion_size).toBeNull()
  })

  it('preserves non-JSON fields', () => {
    const parsed = parseRecipe(makeRecipe({ name: 'Pasta', servings: 6 }))
    expect(parsed.name).toBe('Pasta')
    expect(parsed.servings).toBe(6)
    expect(parsed.is_favorite).toBe(false)
  })

  it('preserves rating value', () => {
    const parsed = parseRecipe(makeRecipe({ rating: 4 }))
    expect(parsed.rating).toBe(4)
  })

  it('handles null rating', () => {
    const parsed = parseRecipe(makeRecipe({ rating: null }))
    expect(parsed.rating).toBeNull()
  })

  it('preserves parent_recipe_id', () => {
    const parsed = parseRecipe(makeRecipe({ parent_recipe_id: 'parent-123' }))
    expect(parsed.parent_recipe_id).toBe('parent-123')
  })

  it('handles null parent_recipe_id', () => {
    const parsed = parseRecipe(makeRecipe({ parent_recipe_id: null }))
    expect(parsed.parent_recipe_id).toBeNull()
  })
})

describe('parseInstructionSteps', () => {
  it('splits a multi-line block into one step per line', () => {
    const steps = parseInstructionSteps('Boil water.\nAdd pasta.\nStir occasionally.')
    expect(steps).toEqual(['Boil water.', 'Add pasta.', 'Stir occasionally.'])
  })

  it('strips leading numbers like "1." or "2)" so they can be re-rendered', () => {
    const steps = parseInstructionSteps('1. Boil water.\n2. Add pasta.\n3) Stir.')
    expect(steps).toEqual(['Boil water.', 'Add pasta.', 'Stir.'])
  })

  it('treats blank lines as separators, not as steps', () => {
    const steps = parseInstructionSteps('Boil water.\n\nAdd pasta.\n\n\nStir.')
    expect(steps).toEqual(['Boil water.', 'Add pasta.', 'Stir.'])
  })

  it('returns a single step when the input has no line breaks', () => {
    const steps = parseInstructionSteps('Just do the thing.')
    expect(steps).toEqual(['Just do the thing.'])
  })

  it('returns an empty array for empty or whitespace-only input', () => {
    expect(parseInstructionSteps('')).toEqual([])
    expect(parseInstructionSteps('   \n  \n')).toEqual([])
  })

  it('trims surrounding whitespace from each step', () => {
    const steps = parseInstructionSteps('  Boil water.  \n  Add pasta.  ')
    expect(steps).toEqual(['Boil water.', 'Add pasta.'])
  })
})
