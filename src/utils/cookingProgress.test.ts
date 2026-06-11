import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  clearCookingProgress,
  cookingProgressKey,
  fingerprintInstructions,
  loadCookingProgress,
  saveCookingProgress,
} from './cookingProgress'

afterEach(() => {
  localStorage.clear()
  vi.restoreAllMocks()
})

const FP = fingerprintInstructions('1. Boil. 2. Stir.')

describe('fingerprintInstructions', () => {
  it('is stable for identical input and differs for changed input', () => {
    expect(fingerprintInstructions('same text')).toBe(fingerprintInstructions('same text'))
    expect(fingerprintInstructions('a')).not.toBe(fingerprintInstructions('b'))
  })

  it('returns a compact string, not the full text', () => {
    const fp = fingerprintInstructions('a'.repeat(5000))
    expect(fp.length).toBeLessThan(15)
  })
})

describe('cookingProgress storage', () => {
  it('round-trips saved progress for a recipe', () => {
    saveCookingProgress('r1', { completedSteps: [0, 2], addedIngredients: [1], fingerprint: FP })
    expect(loadCookingProgress('r1', FP)).toEqual({
      completedSteps: [0, 2],
      addedIngredients: [1],
      fingerprint: FP,
    })
  })

  it('keys progress per recipe so recipes do not share state', () => {
    saveCookingProgress('r1', { completedSteps: [0], addedIngredients: [], fingerprint: FP })
    saveCookingProgress('r2', { completedSteps: [3], addedIngredients: [4], fingerprint: FP })
    expect(loadCookingProgress('r1', FP).completedSteps).toEqual([0])
    expect(loadCookingProgress('r2', FP).completedSteps).toEqual([3])
  })

  it('returns empty progress when nothing is stored', () => {
    expect(loadCookingProgress('missing', FP)).toEqual({
      completedSteps: [],
      addedIngredients: [],
      fingerprint: FP,
    })
  })

  it('discards progress whose fingerprint does not match the current source', () => {
    saveCookingProgress('r1', { completedSteps: [2, 4], addedIngredients: [1], fingerprint: FP })
    // Instructions changed (e.g. enhanced gone after reload, or edited) → the
    // saved indices would land on the wrong steps, so they're discarded.
    const other = fingerprintInstructions('1. Totally different steps.')
    expect(loadCookingProgress('r1', other)).toEqual({
      completedSteps: [],
      addedIngredients: [],
      fingerprint: other,
    })
  })

  it('restores progress when the fingerprint matches', () => {
    saveCookingProgress('r1', { completedSteps: [2], addedIngredients: [0], fingerprint: FP })
    expect(loadCookingProgress('r1', FP).completedSteps).toEqual([2])
  })

  it('clear removes a recipe entry', () => {
    saveCookingProgress('r1', { completedSteps: [0], addedIngredients: [0], fingerprint: FP })
    clearCookingProgress('r1')
    expect(loadCookingProgress('r1', FP)).toEqual({
      completedSteps: [],
      addedIngredients: [],
      fingerprint: FP,
    })
  })

  it('returns empty progress for malformed JSON rather than throwing', () => {
    localStorage.setItem(cookingProgressKey('r1'), '{not json')
    expect(loadCookingProgress('r1', FP)).toEqual({
      completedSteps: [],
      addedIngredients: [],
      fingerprint: FP,
    })
  })

  it('coerces non-array fields to empty arrays', () => {
    localStorage.setItem(
      cookingProgressKey('r1'),
      JSON.stringify({ completedSteps: 'nope', addedIngredients: [2], fingerprint: FP }),
    )
    expect(loadCookingProgress('r1', FP)).toEqual({
      completedSteps: [],
      addedIngredients: [2],
      fingerprint: FP,
    })
  })

  it('load falls back to empty when storage access throws', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('SecurityError')
    })
    expect(loadCookingProgress('r1', FP)).toEqual({
      completedSteps: [],
      addedIngredients: [],
      fingerprint: FP,
    })
  })

  it('save swallows storage-denied errors', () => {
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('QuotaExceeded')
    })
    expect(() =>
      saveCookingProgress('r1', { completedSteps: [0], addedIngredients: [], fingerprint: FP })
    )
      .not.toThrow()
  })
})
