/**
 * Per-recipe cooking-mode progress persisted to localStorage so reloading or
 * briefly walking away mid-cook doesn't wipe the cook's place. Tracks which
 * step cards are checked off and which ingredients have been added.
 *
 * Explicitly exiting cooking mode clears the entry (see {@link clearCookingProgress});
 * a reload restores it.
 */
export interface CookingProgress {
  /** Global indices (across all instruction sections) of completed steps. */
  completedSteps: number[]
  /** Indices into `parsed.ingredients` of ingredients marked as added. */
  addedIngredients: number[]
}

const STORAGE_PREFIX = 'fewd:cooking-progress:'

/** localStorage key for a recipe's cooking progress. */
export function cookingProgressKey(recipeId: string): string {
  return `${STORAGE_PREFIX}${recipeId}`
}

const EMPTY: CookingProgress = { completedSteps: [], addedIngredients: [] }

function isNumberArray(value: unknown): value is number[] {
  return Array.isArray(value) && value.every((n) => typeof n === 'number')
}

/**
 * Reads a recipe's saved progress. Returns empty progress when nothing is
 * stored, the entry is malformed, or storage access is denied (e.g. Safari
 * "Block All Cookies") — persistence is best-effort and never crashes the cook
 * out of their recipe.
 */
export function loadCookingProgress(recipeId: string): CookingProgress {
  try {
    const raw = localStorage.getItem(cookingProgressKey(recipeId))
    if (!raw) return { ...EMPTY }
    const parsed = JSON.parse(raw) as unknown
    if (typeof parsed !== 'object' || parsed === null) return { ...EMPTY }
    const { completedSteps, addedIngredients } = parsed as Record<string, unknown>
    return {
      completedSteps: isNumberArray(completedSteps) ? completedSteps : [],
      addedIngredients: isNumberArray(addedIngredients) ? addedIngredients : [],
    }
  } catch {
    return { ...EMPTY }
  }
}

/** Persists a recipe's progress; ignores storage-denied / quota errors. */
export function saveCookingProgress(recipeId: string, progress: CookingProgress): void {
  try {
    localStorage.setItem(cookingProgressKey(recipeId), JSON.stringify(progress))
  } catch {
    // no-op — persistence is best-effort
  }
}

/** Removes a recipe's saved progress (called on explicit cooking-mode exit). */
export function clearCookingProgress(recipeId: string): void {
  try {
    localStorage.removeItem(cookingProgressKey(recipeId))
  } catch {
    // no-op
  }
}
