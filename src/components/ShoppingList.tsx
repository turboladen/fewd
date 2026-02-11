import { useState } from 'react'
import { useShoppingList } from '../hooks/useShoppingList'
import { formatAmount } from '../types/recipe'
import type { IngredientAmount } from '../types/recipe'
import type { AggregatedIngredient, IngredientSource } from '../types/shopping'
import { addDays, formatDateKey, getMonday, getWeekDates } from '../utils/dates'
import { EmptyState } from './EmptyState'
import { IconArrowLeft, IconArrowRight, IconChevronDown, IconChevronUp } from './Icon'

function formatSourceLabel(source: IngredientSource): string {
  const dateObj = new Date(source.meal_date + 'T00:00:00')
  const dayName = dateObj.toLocaleDateString('en-US', { weekday: 'short' })
  const typeLabel = source.source_type === 'recipe'
    ? source.source_name ?? 'Recipe'
    : 'Ad-hoc'
  return `${dayName} ${source.meal_type} \u2014 ${typeLabel}`
}

function isPartialSource(source: IngredientSource): boolean {
  return source.recipe_servings != null
    && source.person_servings != null
    && source.person_servings < source.recipe_servings
}

function IngredientCard({
  ingredient,
  isExpanded,
  onToggle,
}: {
  ingredient: AggregatedIngredient
  isExpanded: boolean
  onToggle: () => void
}) {
  const hasTotals = ingredient.total_amount !== null && ingredient.total_unit !== null
  const hasPartialRecipe = ingredient.items.some(isPartialSource)

  return (
    <div className='card'>
      <button
        onClick={onToggle}
        className='w-full text-left px-4 py-3 flex items-center justify-between hover:bg-stone-50 rounded-xl transition-colors'
      >
        <div className='flex items-center gap-3'>
          <span className='font-medium text-stone-900'>
            {ingredient.ingredient_name}
          </span>
          {hasTotals && (
            <span className='text-sm text-stone-500'>
              {formatAmount(ingredient.total_amount as IngredientAmount)} {ingredient.total_unit}
            </span>
          )}
          {!hasTotals && ingredient.items.length > 1 && (
            <span className='tag text-amber-600 bg-amber-50'>
              Mixed units
            </span>
          )}
          {hasPartialRecipe && (
            <span className='tag text-amber-600 bg-amber-50'>
              Partial recipe
            </span>
          )}
        </div>
        <div className='flex items-center gap-2'>
          <span className='text-xs text-stone-400'>
            {ingredient.items.length} source{ingredient.items.length !== 1 ? 's' : ''}
          </span>
          <span className='text-stone-400 transition-transform duration-200'>
            {isExpanded
              ? <IconChevronUp className='w-4 h-4' />
              : <IconChevronDown className='w-4 h-4' />}
          </span>
        </div>
      </button>

      {isExpanded && (
        <div className='animate-expand'>
          <div className='px-4 pb-3 border-t border-stone-100'>
            <div className='mt-2 space-y-1'>
              {ingredient.items.map((source, i) => (
                <div key={i} className='flex items-center gap-2 text-sm text-stone-600'>
                  <span className='text-stone-900 font-medium min-w-[80px]'>
                    {formatAmount(source.amount)} {source.unit}
                  </span>
                  <span className='text-stone-400'>{formatSourceLabel(source)}</span>
                  {isPartialSource(source) && (
                    <span className='text-xs text-amber-500'>
                      ({source.person_servings} of {source.recipe_servings} servings)
                    </span>
                  )}
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

export function ShoppingList() {
  const [currentMonday, setCurrentMonday] = useState(() => getMonday(new Date()))
  const [expanded, setExpanded] = useState<Set<string>>(new Set())

  const weekDates = getWeekDates(currentMonday)
  const startDate = formatDateKey(weekDates[0])
  const endDate = formatDateKey(weekDates[6])

  const { data: ingredients, isLoading, error } = useShoppingList(startDate, endDate)

  const prevWeek = () => setCurrentMonday(addDays(currentMonday, -7))
  const nextWeek = () => setCurrentMonday(addDays(currentMonday, 7))
  const goToThisWeek = () => setCurrentMonday(getMonday(new Date()))

  const toggleExpanded = (name: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(name)) {
        next.delete(name)
      } else {
        next.add(name)
      }
      return next
    })
  }

  const expandAll = () => {
    if (ingredients) {
      setExpanded(new Set(ingredients.map((i) => i.ingredient_name)))
    }
  }

  const collapseAll = () => {
    setExpanded(new Set())
  }

  const weekRangeLabel = `${
    weekDates[0].toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
  } - ${
    weekDates[6].toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    })
  }`

  return (
    <div className='p-6 max-w-3xl mx-auto'>
      {/* Header */}
      <div className='flex items-center justify-between mb-6'>
        <h1 className='text-2xl font-bold text-stone-900'>Shopping List</h1>
        <div className='flex items-center gap-3'>
          <button
            onClick={prevWeek}
            className='btn-sm btn-outline'
          >
            <IconArrowLeft className='w-4 h-4 mr-1' /> Prev
          </button>
          <span className='text-sm font-medium text-stone-700 min-w-[180px] text-center'>
            {weekRangeLabel}
          </span>
          <button
            onClick={nextWeek}
            className='btn-sm btn-outline'
          >
            Next <IconArrowRight className='w-4 h-4 ml-1' />
          </button>
          <button
            onClick={goToThisWeek}
            className='btn-sm btn-primary'
          >
            This Week
          </button>
        </div>
      </div>

      {/* Loading state */}
      {isLoading && <div className='text-stone-500 animate-pulse'>Loading shopping list...</div>}

      {/* Error state */}
      {error && (
        <div className='panel-error text-red-700 text-sm'>
          {String(error)}
        </div>
      )}

      {/* Empty state */}
      {!isLoading && !error && ingredients && ingredients.length === 0 && (
        <EmptyState
          emoji='🛒'
          title='Nothing to buy this week'
          description='Plan some meals in the Planner tab to generate your shopping list.'
        />
      )}

      {/* Ingredient list */}
      {ingredients && ingredients.length > 0 && (
        <>
          <div className='flex items-center justify-between mb-3'>
            <span className='text-sm text-stone-500'>
              {ingredients.length} ingredient{ingredients.length !== 1 ? 's' : ''}
            </span>
            <div className='flex gap-2'>
              <button
                onClick={expandAll}
                className='text-xs text-primary-600 hover:underline'
              >
                Expand all
              </button>
              <button
                onClick={collapseAll}
                className='text-xs text-primary-600 hover:underline'
              >
                Collapse all
              </button>
            </div>
          </div>

          <div className='space-y-2'>
            {ingredients.map((ingredient) => (
              <IngredientCard
                key={ingredient.ingredient_name}
                ingredient={ingredient}
                isExpanded={expanded.has(ingredient.ingredient_name)}
                onToggle={() => toggleExpanded(ingredient.ingredient_name)}
              />
            ))}
          </div>
        </>
      )}
    </div>
  )
}
