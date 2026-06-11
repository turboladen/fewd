import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  clearCookingProgress,
  cookingProgressKey,
  loadCookingProgress,
  saveCookingProgress,
} from './cookingProgress'

afterEach(() => {
  localStorage.clear()
  vi.restoreAllMocks()
})

describe('cookingProgress storage', () => {
  it('round-trips saved progress for a recipe', () => {
    saveCookingProgress('r1', { completedSteps: [0, 2], addedIngredients: [1] })
    expect(loadCookingProgress('r1')).toEqual({
      completedSteps: [0, 2],
      addedIngredients: [1],
    })
  })

  it('keys progress per recipe so recipes do not share state', () => {
    saveCookingProgress('r1', { completedSteps: [0], addedIngredients: [] })
    saveCookingProgress('r2', { completedSteps: [3], addedIngredients: [4] })
    expect(loadCookingProgress('r1').completedSteps).toEqual([0])
    expect(loadCookingProgress('r2').completedSteps).toEqual([3])
  })

  it('returns empty progress when nothing is stored', () => {
    expect(loadCookingProgress('missing')).toEqual({
      completedSteps: [],
      addedIngredients: [],
    })
  })

  it('clear removes a recipe entry', () => {
    saveCookingProgress('r1', { completedSteps: [0], addedIngredients: [0] })
    clearCookingProgress('r1')
    expect(loadCookingProgress('r1')).toEqual({
      completedSteps: [],
      addedIngredients: [],
    })
  })

  it('returns empty progress for malformed JSON rather than throwing', () => {
    localStorage.setItem(cookingProgressKey('r1'), '{not json')
    expect(loadCookingProgress('r1')).toEqual({
      completedSteps: [],
      addedIngredients: [],
    })
  })

  it('coerces non-array fields to empty arrays', () => {
    localStorage.setItem(
      cookingProgressKey('r1'),
      JSON.stringify({ completedSteps: 'nope', addedIngredients: [2] }),
    )
    expect(loadCookingProgress('r1')).toEqual({
      completedSteps: [],
      addedIngredients: [2],
    })
  })

  it('load falls back to empty when storage access throws', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('SecurityError')
    })
    expect(loadCookingProgress('r1')).toEqual({
      completedSteps: [],
      addedIngredients: [],
    })
  })

  it('save swallows storage-denied errors', () => {
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('QuotaExceeded')
    })
    expect(() => saveCookingProgress('r1', { completedSteps: [0], addedIngredients: [] }))
      .not.toThrow()
  })
})
