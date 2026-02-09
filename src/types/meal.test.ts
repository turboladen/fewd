import { describe, expect, it } from 'vitest'
import type { Meal } from './meal'
import { parseMeal } from './meal'

describe('parseMeal', () => {
  const makeMeal = (overrides: Partial<Meal> = {}): Meal => ({
    id: 'meal-1',
    date: '2025-06-10',
    meal_type: 'Dinner',
    order_index: 2,
    servings: JSON.stringify([
      {
        food_type: 'recipe',
        person_id: 'p1',
        recipe_id: 'r1',
        servings_count: 1.5,
        notes: null,
      },
      {
        food_type: 'adhoc',
        person_id: 'p2',
        adhoc_items: [
          { name: 'banana', amount: { type: 'single', value: 1 }, unit: 'whole' },
        ],
        notes: 'sliced',
      },
    ]),
    created_at: '2025-01-01T00:00:00Z',
    updated_at: '2025-01-01T00:00:00Z',
    ...overrides,
  })

  it('parses servings from JSON string', () => {
    const parsed = parseMeal(makeMeal())
    expect(parsed.servings).toHaveLength(2)
  })

  it('parses recipe serving', () => {
    const parsed = parseMeal(makeMeal())
    const recipeSrv = parsed.servings[0]
    expect(recipeSrv.food_type).toBe('recipe')
    if (recipeSrv.food_type === 'recipe') {
      expect(recipeSrv.recipe_id).toBe('r1')
      expect(recipeSrv.servings_count).toBe(1.5)
    }
  })

  it('parses adhoc serving with items', () => {
    const parsed = parseMeal(makeMeal())
    const adhocSrv = parsed.servings[1]
    expect(adhocSrv.food_type).toBe('adhoc')
    if (adhocSrv.food_type === 'adhoc') {
      expect(adhocSrv.adhoc_items).toHaveLength(1)
      expect(adhocSrv.adhoc_items[0].name).toBe('banana')
      expect(adhocSrv.notes).toBe('sliced')
    }
  })

  it('preserves non-JSON fields', () => {
    const parsed = parseMeal(makeMeal())
    expect(parsed.date).toBe('2025-06-10')
    expect(parsed.meal_type).toBe('Dinner')
    expect(parsed.order_index).toBe(2)
  })

  it('handles empty servings', () => {
    const parsed = parseMeal(makeMeal({ servings: '[]' }))
    expect(parsed.servings).toEqual([])
  })
})
