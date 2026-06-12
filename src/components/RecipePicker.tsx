import { useEffect, useId, useMemo, useState } from 'react'
import type { RecipePickerOption } from '../types/recipe'

interface Props {
  recipes: RecipePickerOption[]
  /** Selected recipe id, or '' for no selection. */
  value: string
  onChange: (recipeId: string) => void
  /** Slot meal type (Title Case, e.g. 'Dinner') used to boost tag-matching recipes. */
  mealType?: string
  placeholder?: string
  className?: string
}

interface RecipeSection {
  label: string | null
  recipes: RecipePickerOption[]
}

/** Recently-used section is capped; overflow falls through to "All recipes". */
const RECENTLY_USED_LIMIT = 5

function hasMealTypeTag(recipe: RecipePickerOption, mealType?: string): boolean {
  if (!mealType) return false
  const target = mealType.toLowerCase()
  return recipe.tags.some((tag) => tag.toLowerCase() === target)
}

function hasBeenPlanned(recipe: RecipePickerOption): boolean {
  return recipe.times_planned > 0 || recipe.last_planned !== null
}

function byName(a: RecipePickerOption, b: RecipePickerOption): number {
  return a.name.localeCompare(b.name)
}

/** Tag-matching recipes first; 0 when both (or neither) match. */
function byMealTypeBoost(
  a: RecipePickerOption,
  b: RecipePickerOption,
  mealType?: string,
): number {
  return Number(hasMealTypeTag(b, mealType)) - Number(hasMealTypeTag(a, mealType))
}

/** Most recent `last_planned` first (ISO strings compare lexicographically), then most planned. */
function byRecency(a: RecipePickerOption, b: RecipePickerOption): number {
  return (
    (b.last_planned ?? '').localeCompare(a.last_planned ?? '')
    || b.times_planned - a.times_planned
  )
}

function buildSections(
  recipes: RecipePickerOption[],
  mealType?: string,
): RecipeSection[] {
  const favorites = recipes
    .filter((r) => r.is_favorite)
    .sort((a, b) => byMealTypeBoost(a, b, mealType) || byName(a, b))

  const recentlyUsed = recipes
    .filter((r) => !r.is_favorite && hasBeenPlanned(r))
    .sort((a, b) => byMealTypeBoost(a, b, mealType) || byRecency(a, b) || byName(a, b))
  const recentOverflow = recentlyUsed.splice(RECENTLY_USED_LIMIT)

  const others = recipes
    .filter((r) => !r.is_favorite && !hasBeenPlanned(r))
    .concat(recentOverflow)
    .sort((a, b) => byMealTypeBoost(a, b, mealType) || byName(a, b))

  const sections: RecipeSection[] = []
  if (favorites.length > 0) sections.push({ label: 'Favorites', recipes: favorites })
  if (recentlyUsed.length > 0) sections.push({ label: 'Recently used', recipes: recentlyUsed })
  if (others.length > 0) sections.push({ label: 'All recipes', recipes: others })
  return sections
}

function matchScore(recipe: RecipePickerOption, query: string, mealType?: string): number {
  let score = 0
  if (recipe.name.toLowerCase().startsWith(query)) score += 8
  if (hasMealTypeTag(recipe, mealType)) score += 4
  if (recipe.is_favorite) score += 2
  if (hasBeenPlanned(recipe)) score += 1
  return score
}

function rankFiltered(
  recipes: RecipePickerOption[],
  query: string,
  mealType?: string,
): RecipePickerOption[] {
  return recipes
    .filter((r) => r.name.toLowerCase().includes(query))
    .sort((a, b) => matchScore(b, query, mealType) - matchScore(a, query, mealType) || byName(a, b))
}

/**
 * Searchable recipe combobox: typeahead substring filtering with context-aware
 * ranking. Unfiltered, recipes are grouped into Favorites / Recently used /
 * All recipes sections; while typing, matches are shown as one ranked list.
 * Recipes tagged with the slot's `mealType` are boosted in both modes.
 */
export function RecipePicker({
  recipes,
  value,
  onChange,
  mealType,
  placeholder = 'Select recipe...',
  className = '',
}: Props) {
  const [isOpen, setIsOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [highlightedIndex, setHighlightedIndex] = useState(0)
  const baseId = useId()
  const listboxId = `${baseId}-listbox`

  const trimmedQuery = query.trim().toLowerCase()
  const sections = useMemo(
    () =>
      trimmedQuery
        ? [{ label: null, recipes: rankFiltered(recipes, trimmedQuery, mealType) }]
        : buildSections(recipes, mealType),
    [recipes, trimmedQuery, mealType],
  )
  const flatRecipes = useMemo(() => sections.flatMap((s) => s.recipes), [sections])

  const selectedName = recipes.find((r) => r.id === value)?.name ?? ''
  const activeIndex = flatRecipes.length > 0
    ? Math.min(highlightedIndex, flatRecipes.length - 1)
    : -1
  const optionId = (index: number) => `${baseId}-option-${index}`

  useEffect(() => {
    if (!isOpen || activeIndex < 0) return
    // jsdom doesn't implement scrollIntoView; optional call keeps tests happy.
    document.getElementById(`${baseId}-option-${activeIndex}`)?.scrollIntoView?.({
      block: 'nearest',
    })
  }, [isOpen, activeIndex, baseId])

  const openList = () => {
    if (isOpen) return
    setQuery('')
    const selectedIndex = flatRecipes.findIndex((r) => r.id === value)
    setHighlightedIndex(selectedIndex >= 0 ? selectedIndex : 0)
    setIsOpen(true)
  }

  const closeList = () => {
    setIsOpen(false)
    setQuery('')
  }

  const handleSelect = (recipeId: string) => {
    onChange(recipeId)
    closeList()
  }

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const raw = e.target.value
    // Typing while closed (focus kept after Escape/Enter) edits the displayed
    // selected name; start a fresh query from just the typed delta instead.
    const next = !isOpen && selectedName && raw.startsWith(selectedName)
      ? raw.slice(selectedName.length)
      : raw
    setQuery(next)
    setHighlightedIndex(0)
    setIsOpen(true)
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        if (!isOpen) openList()
        else setHighlightedIndex((i) => Math.min(i + 1, flatRecipes.length - 1))
        break
      case 'ArrowUp':
        e.preventDefault()
        if (!isOpen) openList()
        else setHighlightedIndex((i) => Math.max(i - 1, 0))
        break
      case 'Enter':
        if (isOpen && activeIndex >= 0) {
          e.preventDefault()
          handleSelect(flatRecipes[activeIndex].id)
        }
        break
      case 'Escape':
        if (isOpen) {
          e.preventDefault()
          // Keep an outer Escape handler (e.g. MealPlanner's close-editor
          // listener) from also firing while the picker consumes the key.
          e.stopPropagation()
          closeList()
        }
        break
      case 'Tab':
        closeList()
        break
    }
  }

  return (
    <div className={`relative ${className}`}>
      <input
        type='text'
        role='combobox'
        aria-expanded={isOpen}
        aria-controls={listboxId}
        aria-autocomplete='list'
        aria-activedescendant={isOpen && activeIndex >= 0 ? optionId(activeIndex) : undefined}
        aria-label='Recipe'
        className='input-sm w-full'
        value={isOpen ? query : selectedName}
        placeholder={placeholder}
        onFocus={openList}
        onClick={openList}
        onChange={handleInputChange}
        onKeyDown={handleKeyDown}
        onBlur={closeList}
      />
      {isOpen && (
        <div
          id={listboxId}
          role='listbox'
          aria-label='Recipes'
          className='absolute z-20 mt-1 w-full card p-1 max-h-64 overflow-y-auto animate-slide-down'
        >
          {flatRecipes.length === 0
            ? <div className='px-2 py-1.5 text-xs text-stone-400'>No recipes found</div>
            : sections.map((section) => (
              <div
                key={section.label ?? 'results'}
                role='group'
                aria-label={section.label ?? 'Results'}
              >
                {section.label && (
                  <div
                    role='presentation'
                    className='px-2 pt-2 pb-1 text-[10px] font-semibold uppercase tracking-wide text-stone-400'
                  >
                    {section.label}
                  </div>
                )}
                {section.recipes.map((recipe) => {
                  const index = flatRecipes.indexOf(recipe)
                  return (
                    <div
                      key={recipe.id}
                      id={optionId(index)}
                      role='option'
                      aria-selected={recipe.id === value}
                      className={`px-2 py-1.5 text-xs rounded-md cursor-pointer truncate ${
                        index === activeIndex
                          ? 'bg-primary-100 text-primary-900'
                          : 'text-stone-700'
                      }`}
                      onMouseDown={(e) => e.preventDefault()}
                      onClick={() => handleSelect(recipe.id)}
                      onMouseEnter={() => setHighlightedIndex(index)}
                    >
                      {recipe.name}
                    </div>
                  )
                })}
              </div>
            ))}
        </div>
      )}
    </div>
  )
}
