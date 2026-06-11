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
  /**
   * Fingerprint of the instruction source the indices were derived from
   * ({@link fingerprintInstructions}). Step indices only mean something against
   * the exact text that produced them — enhanced vs. original instructions, or
   * an edit between sessions, renumber the steps. On load we discard progress
   * whose fingerprint doesn't match the current source so a saved index never
   * marks (or anchors the cook to) the wrong step.
   */
  fingerprint: string
}

const STORAGE_PREFIX = 'fewd:cooking-progress:'

/** localStorage key for a recipe's cooking progress. */
export function cookingProgressKey(recipeId: string): string {
  return `${STORAGE_PREFIX}${recipeId}`
}

/**
 * Small stable hash (djb2, xor variant) of the instruction source, stored
 * alongside progress so we can detect when the steps a saved index refers to
 * have changed. Returned as an unsigned base-36 string to keep the entry tiny —
 * storing the full text would bloat localStorage for no benefit.
 */
export function fingerprintInstructions(source: string): string {
  let hash = 5381
  for (let i = 0; i < source.length; i++) {
    hash = ((hash << 5) + hash) ^ source.charCodeAt(i)
  }
  return (hash >>> 0).toString(36)
}

const empty = (fingerprint: string): CookingProgress => ({
  completedSteps: [],
  addedIngredients: [],
  fingerprint,
})

function isNumberArray(value: unknown): value is number[] {
  return Array.isArray(value) && value.every((n) => typeof n === 'number')
}

/**
 * Reads a recipe's saved progress, scoped to `expectedFingerprint`. Returns
 * empty progress when nothing is stored, the entry is malformed, the stored
 * fingerprint doesn't match the current instruction source (steps changed), or
 * storage access is denied (e.g. Safari "Block All Cookies") — persistence is
 * best-effort and never crashes the cook out of their recipe.
 */
export function loadCookingProgress(
  recipeId: string,
  expectedFingerprint: string,
): CookingProgress {
  try {
    const raw = localStorage.getItem(cookingProgressKey(recipeId))
    if (!raw) return empty(expectedFingerprint)
    const parsed = JSON.parse(raw) as unknown
    if (typeof parsed !== 'object' || parsed === null) return empty(expectedFingerprint)
    const { completedSteps, addedIngredients, fingerprint } = parsed as Record<string, unknown>
    // Stale against the current instruction text — discard so indices can't
    // land on the wrong steps after renumbering.
    if (fingerprint !== expectedFingerprint) return empty(expectedFingerprint)
    return {
      completedSteps: isNumberArray(completedSteps) ? completedSteps : [],
      addedIngredients: isNumberArray(addedIngredients) ? addedIngredients : [],
      fingerprint: expectedFingerprint,
    }
  } catch {
    return empty(expectedFingerprint)
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
