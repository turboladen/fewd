import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  clearCookingProgress,
  loadCookingProgress,
  saveCookingProgress,
} from '../utils/cookingProgress'

export interface CookingProgressState {
  /** Whether the step at the given global index is checked off. */
  isStepComplete: (index: number) => boolean
  /** Whether the ingredient at the given index is marked as added. */
  isIngredientAdded: (index: number) => boolean
  /**
   * Global index of the current step — the topmost not-yet-completed step, or
   * `null` once every step is done. Drives the cook's "where am I now?" anchor.
   */
  currentStepIndex: number | null
  toggleStep: (index: number) => void
  toggleIngredient: (index: number) => void
  /** Clears persisted progress and resets in-memory state (called on exit). */
  reset: () => void
}

function toggle(set: Set<number>, index: number): Set<number> {
  const next = new Set(set)
  if (next.has(index)) {
    next.delete(index)
  } else {
    next.add(index)
  }
  return next
}

/**
 * Tracks cooking-mode check-off state (completed steps + added ingredients) for
 * one recipe, persisting to localStorage so a mid-cook reload restores place.
 * `totalSteps` bounds the current-step search. `fingerprint` identifies the
 * instruction source the step indices are derived from
 * ({@link fingerprintInstructions}); stored progress whose fingerprint doesn't
 * match is discarded on load, so a saved index never marks the wrong step after
 * the instructions change (enhanced vs. original, or an edit between sessions).
 *
 * Progress is keyed by `recipeId`; switching recipes loads that recipe's saved
 * state. A runtime fingerprint change (e.g. AI-enhanced instructions arriving
 * after the cook started) intentionally does NOT wipe live in-memory progress —
 * subsequent saves simply adopt the new fingerprint; the load-time guard only
 * protects the reload/edit path. {@link CookingProgressState.reset} clears
 * persistence and is wired to explicit cooking-mode exit (a reload, which does
 * not call reset, restores).
 */
export function useCookingProgress(
  recipeId: string,
  totalSteps: number,
  fingerprint: string,
): CookingProgressState {
  // One read on first mount — a single storage snapshot seeds both sets so they
  // can never hydrate from inconsistent reads.
  const [initial] = useState(() => loadCookingProgress(recipeId, fingerprint))
  const [completedSteps, setCompletedSteps] = useState(() => new Set(initial.completedSteps))
  const [addedIngredients, setAddedIngredients] = useState(() => new Set(initial.addedIngredients))

  // Re-hydrate from storage when the recipe changes so each recipe keeps its
  // own progress. Resetting during render (React's "adjusting state when a prop
  // changes" pattern) avoids a render-then-effect flash of the prior recipe's
  // state — and the lazy initializer already covers the first mount. Keyed on
  // recipeId only (not fingerprint) so an enhanced-instructions swap mid-cook
  // doesn't discard the progress the cook has already made.
  const [loadedRecipeId, setLoadedRecipeId] = useState(recipeId)
  if (loadedRecipeId !== recipeId) {
    const saved = loadCookingProgress(recipeId, fingerprint)
    setLoadedRecipeId(recipeId)
    setCompletedSteps(new Set(saved.completedSteps))
    setAddedIngredients(new Set(saved.addedIngredients))
  }

  useEffect(() => {
    // An all-empty state clears the entry rather than writing `{[],[]}`. This
    // both avoids littering localStorage with a key for every recipe ever
    // opened, and keeps the invariant "empty in memory == no stored entry" — so
    // a cook who checks a step then unchecks it doesn't resurrect it on reload.
    // clearCookingProgress is a no-op when nothing is stored.
    if (completedSteps.size === 0 && addedIngredients.size === 0) {
      clearCookingProgress(recipeId)
      return
    }
    saveCookingProgress(recipeId, {
      completedSteps: [...completedSteps],
      addedIngredients: [...addedIngredients],
      fingerprint,
    })
  }, [recipeId, fingerprint, completedSteps, addedIngredients])

  const toggleStep = useCallback((index: number) => {
    setCompletedSteps((prev) => toggle(prev, index))
  }, [])

  const toggleIngredient = useCallback((index: number) => {
    setAddedIngredients((prev) => toggle(prev, index))
  }, [])

  const reset = useCallback(() => {
    clearCookingProgress(recipeId)
    setCompletedSteps(new Set())
    setAddedIngredients(new Set())
  }, [recipeId])

  const currentStepIndex = useMemo(() => {
    for (let i = 0; i < totalSteps; i++) {
      if (!completedSteps.has(i)) return i
    }
    return null
  }, [completedSteps, totalSteps])

  return {
    isStepComplete: useCallback((index: number) => completedSteps.has(index), [completedSteps]),
    isIngredientAdded: useCallback(
      (index: number) => addedIngredients.has(index),
      [addedIngredients],
    ),
    currentStepIndex,
    toggleStep,
    toggleIngredient,
    reset,
  }
}
