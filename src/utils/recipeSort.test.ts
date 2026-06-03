import { describe, expect, it } from 'vitest'
import { makeRecipe } from '../test/factories'
import { DEFAULT_RECIPE_SORT, isRecipeSortBy, RECIPE_SORT_OPTIONS, sortRecipes } from './recipeSort'

/** Sort and return just the names, in order — keeps assertions readable. */
function names(
  recipes: Parameters<typeof sortRecipes>[0],
  sortBy: Parameters<typeof sortRecipes>[1],
) {
  return sortRecipes(recipes, sortBy).map((r) => r.name)
}

describe('RECIPE_SORT_OPTIONS / metadata', () => {
  it('default is name-asc and is a valid option', () => {
    expect(DEFAULT_RECIPE_SORT).toBe('name-asc')
    expect(RECIPE_SORT_OPTIONS.some((o) => o.value === DEFAULT_RECIPE_SORT)).toBe(true)
  })

  it('every option value is a recognized sort key with a non-empty label', () => {
    for (const opt of RECIPE_SORT_OPTIONS) {
      expect(isRecipeSortBy(opt.value)).toBe(true)
      expect(opt.label.length).toBeGreaterThan(0)
    }
  })
})

describe('isRecipeSortBy', () => {
  it('accepts known values', () => {
    expect(isRecipeSortBy('name-asc')).toBe(true)
    expect(isRecipeSortBy('rating-desc')).toBe(true)
  })

  it('rejects unknown / malformed / non-string values', () => {
    expect(isRecipeSortBy('bogus')).toBe(false)
    expect(isRecipeSortBy(null)).toBe(false)
    expect(isRecipeSortBy(undefined)).toBe(false)
    expect(isRecipeSortBy(42)).toBe(false)
  })
})

describe('sortRecipes — name', () => {
  const recipes = [
    makeRecipe({ id: 'b', name: 'Banana' }),
    makeRecipe({ id: 'a', name: 'Apple' }),
    makeRecipe({ id: 'c', name: 'Cherry' }),
  ]

  it('name-asc orders A→Z', () => {
    expect(names(recipes, 'name-asc')).toEqual(['Apple', 'Banana', 'Cherry'])
  })

  it('name-desc orders Z→A', () => {
    expect(names(recipes, 'name-desc')).toEqual(['Cherry', 'Banana', 'Apple'])
  })
})

describe('sortRecipes — rating-desc (NULLs last)', () => {
  it('descends by rating and puts unrated recipes last', () => {
    const recipes = [
      makeRecipe({ id: '1', name: 'Mid', rating: 3 }),
      makeRecipe({ id: '2', name: 'Unrated', rating: null }),
      makeRecipe({ id: '3', name: 'Top', rating: 5 }),
    ]
    expect(names(recipes, 'rating-desc')).toEqual(['Top', 'Mid', 'Unrated'])
  })
})

describe('sortRecipes — last_planned', () => {
  it('recently planned (desc) puts most-recent first and never-planned last', () => {
    const recipes = [
      makeRecipe({ id: '1', name: 'Old', last_planned: '2026-01-01T00:00:00Z' }),
      makeRecipe({ id: '2', name: 'Never', last_planned: null }),
      makeRecipe({ id: '3', name: 'Recent', last_planned: '2026-05-01T00:00:00Z' }),
    ]
    expect(names(recipes, 'last_planned-desc')).toEqual(['Recent', 'Old', 'Never'])
  })

  it('not-planned-in-a-while (asc) bubbles never-planned recipes to the TOP', () => {
    const recipes = [
      makeRecipe({ id: '1', name: 'Recent', last_planned: '2026-05-01T00:00:00Z' }),
      makeRecipe({ id: '2', name: 'Never', last_planned: null }),
      makeRecipe({ id: '3', name: 'Old', last_planned: '2026-01-01T00:00:00Z' }),
    ]
    expect(names(recipes, 'last_planned-asc')).toEqual(['Never', 'Old', 'Recent'])
  })
})

describe('sortRecipes — times_planned-desc', () => {
  it('most-planned first, zero last', () => {
    const recipes = [
      makeRecipe({ id: '1', name: 'Some', times_planned: 2 }),
      makeRecipe({ id: '2', name: 'None', times_planned: 0 }),
      makeRecipe({ id: '3', name: 'Lots', times_planned: 9 }),
    ]
    expect(names(recipes, 'times_planned-desc')).toEqual(['Lots', 'Some', 'None'])
  })
})

describe('sortRecipes — created_at-desc', () => {
  it('newest first', () => {
    const recipes = [
      makeRecipe({ id: '1', name: 'Older', created_at: '2026-01-01T00:00:00Z' }),
      makeRecipe({ id: '2', name: 'Newest', created_at: '2026-06-01T00:00:00Z' }),
      makeRecipe({ id: '3', name: 'Middle', created_at: '2026-03-01T00:00:00Z' }),
    ]
    expect(names(recipes, 'created_at-desc')).toEqual(['Newest', 'Middle', 'Older'])
  })
})

describe('sortRecipes — favorite-desc', () => {
  it('favorites first, name-ASC within each group', () => {
    const recipes = [
      makeRecipe({ id: '1', name: 'Zucchini', is_favorite: false }),
      makeRecipe({ id: '2', name: 'Waffles', is_favorite: true }),
      makeRecipe({ id: '3', name: 'Apple', is_favorite: false }),
      makeRecipe({ id: '4', name: 'Bacon', is_favorite: true }),
    ]
    expect(names(recipes, 'favorite-desc')).toEqual(['Bacon', 'Waffles', 'Apple', 'Zucchini'])
  })
})

describe('sortRecipes — tiebreaker', () => {
  it('falls back to name-ASC when the primary key is equal (no shimmer)', () => {
    const recipes = [
      makeRecipe({ id: '1', name: 'Beta', rating: 4, last_planned: null }),
      makeRecipe({ id: '2', name: 'Alpha', rating: 4, last_planned: null }),
    ]
    // Same rating → name-ASC decides.
    expect(names(recipes, 'rating-desc')).toEqual(['Alpha', 'Beta'])
  })
})

describe('sortRecipes — purity', () => {
  it('does not mutate or reorder the input array', () => {
    const recipes = [
      makeRecipe({ id: '1', name: 'Banana' }),
      makeRecipe({ id: '2', name: 'Apple' }),
    ]
    const before = recipes.map((r) => r.name)
    sortRecipes(recipes, 'name-asc')
    expect(recipes.map((r) => r.name)).toEqual(before)
  })
})
