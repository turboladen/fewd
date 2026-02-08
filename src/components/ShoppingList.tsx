import { useState } from 'react'
import { useShoppingList } from '../hooks/useShoppingList'
import { formatAmount } from '../types/recipe'
import type { IngredientAmount } from '../types/recipe'
import type { AggregatedIngredient, IngredientSource } from '../types/shopping'
import { addDays, formatDateKey, getMonday, getWeekDates } from '../utils/dates'

function formatSourceLabel(source: IngredientSource): string {
  const dateObj = new Date(source.meal_date + 'T00:00:00')
  const dayName = dateObj.toLocaleDateString('en-US', { weekday: 'short' })
  const typeLabel = source.source_type === 'recipe'
    ? source.source_name ?? 'Recipe'
    : 'Ad-hoc'
  return `${dayName} ${source.meal_type} \u2014 ${typeLabel}`
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

  return (
    <div className='border border-gray-200 rounded-lg bg-white'>
      <button
        onClick={onToggle}
        className='w-full text-left px-4 py-3 flex items-center justify-between hover:bg-gray-50'
      >
        <div className='flex items-center gap-3'>
          <span className='font-medium text-gray-900'>
            {ingredient.ingredient_name}
          </span>
          {hasTotals && (
            <span className='text-sm text-gray-500'>
              {formatAmount(ingredient.total_amount as IngredientAmount)} {ingredient.total_unit}
            </span>
          )}
          {!hasTotals && ingredient.items.length > 1 && (
            <span className='text-xs text-amber-600 bg-amber-50 px-2 py-0.5 rounded'>
              Mixed units
            </span>
          )}
        </div>
        <div className='flex items-center gap-2'>
          <span className='text-xs text-gray-400'>
            {ingredient.items.length} source{ingredient.items.length !== 1 ? 's' : ''}
          </span>
          <span className='text-gray-400 text-sm'>
            {isExpanded ? '\u25B2' : '\u25BC'}
          </span>
        </div>
      </button>

      {isExpanded && (
        <div className='px-4 pb-3 border-t border-gray-100'>
          <div className='mt-2 space-y-1'>
            {ingredient.items.map((source, i) => (
              <div key={i} className='flex items-center gap-2 text-sm text-gray-600'>
                <span className='text-gray-900 font-medium min-w-[80px]'>
                  {formatAmount(source.amount)} {source.unit}
                </span>
                <span className='text-gray-400'>{formatSourceLabel(source)}</span>
              </div>
            ))}
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
        <h1 className='text-2xl font-bold text-gray-900'>Shopping List</h1>
        <div className='flex items-center gap-3'>
          <button
            onClick={prevWeek}
            className='px-3 py-1 border border-gray-300 rounded text-sm hover:bg-gray-50'
          >
            {'\u2190'} Prev
          </button>
          <span className='text-sm font-medium text-gray-700 min-w-[180px] text-center'>
            {weekRangeLabel}
          </span>
          <button
            onClick={nextWeek}
            className='px-3 py-1 border border-gray-300 rounded text-sm hover:bg-gray-50'
          >
            Next {'\u2192'}
          </button>
          <button
            onClick={goToThisWeek}
            className='px-3 py-1 bg-blue-600 text-white rounded text-sm hover:bg-blue-700'
          >
            This Week
          </button>
        </div>
      </div>

      {/* Loading state */}
      {isLoading && <div className='text-gray-500'>Loading shopping list...</div>}

      {/* Error state */}
      {error && (
        <div className='text-red-600 text-sm bg-red-50 border border-red-200 rounded p-3'>
          {String(error)}
        </div>
      )}

      {/* Empty state */}
      {!isLoading && !error && ingredients && ingredients.length === 0 && (
        <div className='text-center py-12'>
          <p className='text-gray-500 text-lg mb-2'>No meals planned for this week</p>
          <p className='text-gray-400 text-sm'>
            Plan some meals in the Planner tab to see ingredients here.
          </p>
        </div>
      )}

      {/* Ingredient list */}
      {ingredients && ingredients.length > 0 && (
        <>
          <div className='flex items-center justify-between mb-3'>
            <span className='text-sm text-gray-500'>
              {ingredients.length} ingredient{ingredients.length !== 1 ? 's' : ''}
            </span>
            <div className='flex gap-2'>
              <button
                onClick={expandAll}
                className='text-xs text-blue-600 hover:underline'
              >
                Expand all
              </button>
              <button
                onClick={collapseAll}
                className='text-xs text-blue-600 hover:underline'
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
