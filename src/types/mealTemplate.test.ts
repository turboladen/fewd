import { describe, expect, it } from 'vitest'
import type { MealTemplate } from './mealTemplate'
import { parseMealTemplate } from './mealTemplate'

describe('parseMealTemplate', () => {
  const makeTemplate = (overrides: Partial<MealTemplate> = {}): MealTemplate => ({
    id: 'tmpl-1',
    name: 'Weeknight Pasta',
    meal_type: 'Dinner',
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
          { name: 'salad', amount: { type: 'single', value: 1 }, unit: 'bowl' },
        ],
        notes: null,
      },
    ]),
    created_at: '2025-01-01T00:00:00Z',
    updated_at: '2025-01-01T00:00:00Z',
    ...overrides,
  })

  it('parses servings from JSON string', () => {
    const parsed = parseMealTemplate(makeTemplate())
    expect(parsed.servings).toHaveLength(2)
  })

  it('parses recipe serving in template', () => {
    const parsed = parseMealTemplate(makeTemplate())
    const recipeSrv = parsed.servings[0]
    expect(recipeSrv.food_type).toBe('recipe')
    if (recipeSrv.food_type === 'recipe') {
      expect(recipeSrv.recipe_id).toBe('r1')
      expect(recipeSrv.servings_count).toBe(1.5)
    }
  })

  it('parses adhoc serving in template', () => {
    const parsed = parseMealTemplate(makeTemplate())
    const adhocSrv = parsed.servings[1]
    expect(adhocSrv.food_type).toBe('adhoc')
    if (adhocSrv.food_type === 'adhoc') {
      expect(adhocSrv.adhoc_items).toHaveLength(1)
      expect(adhocSrv.adhoc_items[0].name).toBe('salad')
    }
  })

  it('preserves non-JSON fields', () => {
    const parsed = parseMealTemplate(makeTemplate())
    expect(parsed.id).toBe('tmpl-1')
    expect(parsed.name).toBe('Weeknight Pasta')
    expect(parsed.meal_type).toBe('Dinner')
    expect(parsed.created_at).toBe('2025-01-01T00:00:00Z')
  })

  it('handles empty servings', () => {
    const parsed = parseMealTemplate(makeTemplate({ servings: '[]' }))
    expect(parsed.servings).toEqual([])
  })
})
