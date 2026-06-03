import type { Recipe } from '../types/recipe'

/**
 * Client-side sort keys for the Recipes tab. The value encodes both the field
 * and direction (`<field>-<dir>`) so it round-trips cleanly through localStorage.
 *
 * `times_planned`/`last_planned` track *planning*, not cooking — cooking history
 * is deferred (fewd-sx3). Labels say "planned" to match the data.
 */
export type RecipeSortBy =
  | 'name-asc'
  | 'name-desc'
  | 'rating-desc'
  | 'times_planned-desc'
  | 'last_planned-desc'
  | 'last_planned-asc'
  | 'created_at-desc'
  | 'favorite-desc'

export const DEFAULT_RECIPE_SORT: RecipeSortBy = 'name-asc'

/** Single source of truth for the dropdown: option value ↔ user-facing label. */
export const RECIPE_SORT_OPTIONS: { value: RecipeSortBy; label: string }[] = [
  { value: 'name-asc', label: 'Name (A–Z)' },
  { value: 'name-desc', label: 'Name (Z–A)' },
  { value: 'rating-desc', label: 'Highest rated' },
  { value: 'favorite-desc', label: 'Favorites first' },
  { value: 'created_at-desc', label: 'Newly added' },
  { value: 'times_planned-desc', label: 'Most-planned' },
  { value: 'last_planned-desc', label: 'Recently planned' },
  { value: 'last_planned-asc', label: 'Not planned in a while' },
]

const VALID_SORTS = new Set<string>(RECIPE_SORT_OPTIONS.map((o) => o.value))

/** Validates a value (e.g. a localStorage read) is a recognized sort key. */
export function isRecipeSortBy(value: unknown): value is RecipeSortBy {
  return typeof value === 'string' && VALID_SORTS.has(value)
}

type Comparator = (a: Recipe, b: Recipe) => number

/** Shared tiebreaker so equal primary keys resolve deterministically (no shimmer). */
const byNameAsc: Comparator = (a, b) => a.name.localeCompare(b.name)

/** Chain a primary comparator with the name-ASC tiebreaker. */
function withNameTiebreaker(primary: Comparator): Comparator {
  return (a, b) => primary(a, b) || byNameAsc(a, b)
}

/**
 * Compare a nullable field, with explicit null placement.
 * `nulls: 'last'` for the "best/most-recent first" sorts; `nulls: 'first'` for
 * "not planned in a while" so never-planned recipes surface at the top.
 */
function compareNullable<T extends number | string>(
  a: T | null,
  b: T | null,
  dir: 'asc' | 'desc',
  nulls: 'first' | 'last',
): number {
  if (a === null && b === null) return 0
  if (a === null) return nulls === 'first' ? -1 : 1
  if (b === null) return nulls === 'first' ? 1 : -1
  const cmp = a < b ? -1 : a > b ? 1 : 0
  return dir === 'asc' ? cmp : -cmp
}

const COMPARATORS: Record<RecipeSortBy, Comparator> = {
  // Name sorts need no tiebreaker — name IS the primary key.
  'name-asc': byNameAsc,
  'name-desc': (a, b) => byNameAsc(b, a),
  'rating-desc': withNameTiebreaker((a, b) => compareNullable(a.rating, b.rating, 'desc', 'last')),
  'times_planned-desc': withNameTiebreaker((a, b) => b.times_planned - a.times_planned),
  'last_planned-desc': withNameTiebreaker((a, b) =>
    compareNullable(a.last_planned, b.last_planned, 'desc', 'last')
  ),
  'last_planned-asc': withNameTiebreaker((a, b) =>
    compareNullable(a.last_planned, b.last_planned, 'asc', 'first')
  ),
  'created_at-desc': withNameTiebreaker((a, b) => b.created_at.localeCompare(a.created_at)),
  'favorite-desc': withNameTiebreaker((a, b) => Number(b.is_favorite) - Number(a.is_favorite)),
}

/** Returns a new sorted array; never mutates the input (React Query cache is shared). */
export function sortRecipes(recipes: Recipe[], sortBy: RecipeSortBy): Recipe[] {
  return [...recipes].sort(COMPARATORS[sortBy])
}
