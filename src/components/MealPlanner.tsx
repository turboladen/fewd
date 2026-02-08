import { useEffect, useState } from 'react'
import {
  useCreateMeal,
  useDeleteMeal,
  useMealsForDateRange,
  useUpdateMeal,
} from '../hooks/useMeals'
import { usePeople } from '../hooks/usePeople'
import { useRecipes } from '../hooks/useRecipes'
import type { CreateMealDto, ParsedMeal, PersonServing } from '../types/meal'
import { parseMeal } from '../types/meal'
import type { Person } from '../types/person'
import type { Ingredient } from '../types/recipe'
import {
  addDays,
  formatDateDisplay,
  formatDateKey,
  getMonday,
  getWeekDates,
  isToday,
} from '../utils/dates'
import { IngredientInput } from './IngredientInput'

const DEFAULT_MEALS = [
  { type: 'Breakfast', order: 0 },
  { type: 'Lunch', order: 1 },
  { type: 'Dinner', order: 2 },
]

// --- PersonServingEditor ---

function PersonServingEditor({
  person,
  serving,
  recipes,
  onChange,
}: {
  person: Person
  serving: PersonServing | undefined
  recipes: { id: string; name: string }[]
  onChange: (serving: PersonServing | undefined) => void
}) {
  const mode: 'skip' | 'recipe' | 'adhoc' = !serving
    ? 'skip'
    : serving.food_type === 'recipe'
    ? 'recipe'
    : 'adhoc'

  const setMode = (newMode: 'skip' | 'recipe' | 'adhoc') => {
    if (newMode === 'skip') {
      onChange(undefined)
    } else if (newMode === 'recipe') {
      onChange({
        food_type: 'recipe',
        person_id: person.id,
        recipe_id: recipes[0]?.id ?? '',
        servings_count: 1,
        notes: null,
      })
    } else {
      onChange({
        food_type: 'adhoc',
        person_id: person.id,
        adhoc_items: [],
        notes: null,
      })
    }
  }

  return (
    <div className='border border-gray-200 rounded p-3 bg-gray-50'>
      <div className='flex items-center gap-3 mb-2'>
        <span className='font-medium text-sm w-24'>{person.name}</span>
        <div className='flex gap-1'>
          {(['skip', 'recipe', 'adhoc'] as const).map((m) => (
            <button
              key={m}
              type='button'
              onClick={() => setMode(m)}
              className={`text-xs px-2 py-1 rounded border ${
                mode === m
                  ? 'bg-blue-50 border-blue-300 text-blue-700'
                  : 'bg-white border-gray-300 text-gray-500 hover:border-blue-300'
              }`}
            >
              {m === 'skip' ? 'Skip' : m === 'recipe' ? 'Recipe' : 'Ad-hoc'}
            </button>
          ))}
        </div>
      </div>

      {mode === 'recipe' && serving?.food_type === 'recipe' && (
        <div className='flex gap-2 items-center ml-27'>
          <select
            value={serving.recipe_id}
            onChange={(e) => onChange({ ...serving, recipe_id: e.target.value })}
            className='border border-gray-300 p-1 rounded text-sm flex-1'
          >
            <option value=''>Select recipe...</option>
            {recipes.map((r) => <option key={r.id} value={r.id}>{r.name}</option>)}
          </select>
          <input
            type='number'
            min='0.1'
            step='0.1'
            value={serving.servings_count}
            onChange={(e) =>
              onChange({ ...serving, servings_count: parseFloat(e.target.value) || 0.1 })}
            className='border border-gray-300 p-1 rounded w-16 text-sm'
            title='Servings count'
          />
          <span className='text-xs text-gray-500'>servings</span>
        </div>
      )}

      {mode === 'adhoc' && serving?.food_type === 'adhoc' && (
        <div className='ml-27'>
          <IngredientInput
            value={serving.adhoc_items}
            onChange={(items: Ingredient[]) => onChange({ ...serving, adhoc_items: items })}
          />
        </div>
      )}
    </div>
  )
}

// --- MealSlot ---

function MealSlot({
  mealType,
  meal,
  people,
  recipeNames,
  onClick,
}: {
  mealType: string
  meal: ParsedMeal | undefined
  people: Person[]
  recipeNames: Map<string, string>
  onClick: () => void
}) {
  const hasServings = meal && meal.servings.length > 0

  return (
    <button
      onClick={onClick}
      className={`w-full text-left p-2 rounded border text-xs ${
        hasServings
          ? 'bg-white border-gray-200 hover:border-blue-300'
          : 'bg-gray-50 border-dashed border-gray-300 hover:border-blue-300 hover:bg-blue-50'
      }`}
    >
      <div className='font-medium text-gray-700 mb-1'>{mealType}</div>
      {hasServings
        ? (
          <div className='space-y-0.5'>
            {meal.servings.map((s, i) => {
              const personName = people.find((p) => p.id === s.person_id)?.name ?? '?'
              const food = s.food_type === 'recipe'
                ? (recipeNames.get(s.recipe_id) ?? 'Unknown recipe')
                : `${s.adhoc_items.length} item${s.adhoc_items.length !== 1 ? 's' : ''}`
              return (
                <div key={i} className='text-gray-500 truncate'>
                  <span className='text-gray-700'>{personName}</span>: {food}
                </div>
              )
            })}
          </div>
        )
        : <div className='text-gray-400'>+ Plan</div>}
    </button>
  )
}

// --- MealEditor ---

function MealEditor({
  date,
  mealType,
  orderIndex,
  existingMeal,
  people,
  recipes,
  onSave,
  onCancel,
  onDelete,
}: {
  date: string
  mealType: string
  orderIndex: number
  existingMeal: ParsedMeal | undefined
  people: Person[]
  recipes: { id: string; name: string }[]
  onSave: (data: CreateMealDto) => void
  onCancel: () => void
  onDelete?: () => void
}) {
  const [servingsMap, setServingsMap] = useState<Map<string, PersonServing>>(() => {
    const map = new Map<string, PersonServing>()
    if (existingMeal) {
      for (const s of existingMeal.servings) {
        map.set(s.person_id, s)
      }
    }
    return map
  })
  const [confirmingDelete, setConfirmingDelete] = useState(false)
  const [customMealType, setCustomMealType] = useState(mealType)
  const [validationError, setValidationError] = useState<string | null>(null)
  const isCustom = orderIndex >= 3

  const handlePersonChange = (personId: string, serving: PersonServing | undefined) => {
    const newMap = new Map(servingsMap)
    if (serving) {
      newMap.set(personId, serving)
    } else {
      newMap.delete(personId)
    }
    setServingsMap(newMap)
  }

  const handleSave = () => {
    if (servingsMap.size === 0) {
      setValidationError('Assign food to at least one person')
      return
    }
    setValidationError(null)
    const servings = Array.from(servingsMap.values())
    onSave({
      date,
      meal_type: isCustom ? customMealType : mealType,
      order_index: orderIndex,
      servings,
    })
  }

  const displayDate = new Date(date + 'T00:00:00')
  const dateLabel = displayDate.toLocaleDateString('en-US', {
    weekday: 'long',
    month: 'short',
    day: 'numeric',
  })

  return (
    <div className='border border-blue-200 rounded-lg p-4 bg-white'>
      <div className='flex items-center justify-between mb-4'>
        <div>
          <h3 className='font-semibold text-lg'>
            {isCustom
              ? (
                <input
                  type='text'
                  value={customMealType}
                  onChange={(e) => setCustomMealType(e.target.value)}
                  className='border border-gray-300 px-2 py-1 rounded text-lg font-semibold'
                  placeholder='Meal name (e.g. Snack)'
                />
              )
              : mealType}
          </h3>
          <p className='text-sm text-gray-500'>{dateLabel}</p>
        </div>
        <div className='flex gap-2 items-center'>
          {onDelete && (
            confirmingDelete
              ? (
                <span className='flex gap-1 items-center text-sm'>
                  <span className='text-red-600'>Delete?</span>
                  <button
                    onClick={onDelete}
                    className='text-red-700 font-semibold hover:underline'
                  >
                    Yes
                  </button>
                  <button
                    onClick={() => setConfirmingDelete(false)}
                    className='text-gray-500 hover:underline'
                  >
                    No
                  </button>
                </span>
              )
              : (
                <button
                  onClick={() => setConfirmingDelete(true)}
                  className='text-red-600 text-sm hover:underline'
                >
                  Delete
                </button>
              )
          )}
        </div>
      </div>

      <div className='space-y-2 mb-4'>
        {people.map((person) => (
          <PersonServingEditor
            key={person.id}
            person={person}
            serving={servingsMap.get(person.id)}
            recipes={recipes}
            onChange={(s) => handlePersonChange(person.id, s)}
          />
        ))}
      </div>

      {validationError && (
        <div className='mb-2 bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
          {validationError}
        </div>
      )}

      <div className='flex gap-2'>
        <button
          onClick={handleSave}
          className='bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700'
        >
          {existingMeal ? 'Save Changes' : 'Create Meal'}
        </button>
        <button
          onClick={onCancel}
          className='border border-gray-300 px-4 py-2 rounded text-sm text-gray-600 hover:bg-gray-50'
        >
          Cancel
        </button>
      </div>
    </div>
  )
}

// --- Main Component ---

export function MealPlanner() {
  const [currentMonday, setCurrentMonday] = useState(() => getMonday(new Date()))
  const [editingSlot, setEditingSlot] = useState<
    {
      date: string
      mealType: string
      orderIndex: number
    } | null
  >(null)

  const weekDates = getWeekDates(currentMonday)
  const startDate = formatDateKey(weekDates[0])
  const endDate = formatDateKey(weekDates[6])

  const { data: meals, isLoading: mealsLoading, error: mealsError } = useMealsForDateRange(
    startDate,
    endDate,
  )
  const { data: people } = usePeople()
  const { data: rawRecipes } = useRecipes()

  const createMutation = useCreateMeal()
  const updateMutation = useUpdateMeal()
  const deleteMutation = useDeleteMeal()

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && editingSlot) {
        setEditingSlot(null)
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [editingSlot])

  // Build recipe name lookup
  const recipes = rawRecipes?.map((r) => ({ id: r.id, name: r.name })) ?? []
  const recipeNames = new Map(recipes.map((r) => [r.id, r.name]))

  // Parse and group meals by date
  const parsedMeals = meals?.map(parseMeal) ?? []
  const mealsByDate = new Map<string, ParsedMeal[]>()
  for (const m of parsedMeals) {
    const key = m.date
    if (!mealsByDate.has(key)) {
      mealsByDate.set(key, [])
    }
    mealsByDate.get(key)!.push(m)
  }

  const activePeople = people?.filter((p) => p.is_active) ?? []

  const prevWeek = () => setCurrentMonday(addDays(currentMonday, -7))
  const nextWeek = () => setCurrentMonday(addDays(currentMonday, 7))
  const goToToday = () => setCurrentMonday(getMonday(new Date()))

  const handleSave = (data: CreateMealDto) => {
    if (!editingSlot) return

    const dateKey = editingSlot.date
    const dateMeals = mealsByDate.get(dateKey) ?? []
    const existing = dateMeals.find(
      (m) => m.meal_type === editingSlot.mealType && m.order_index === editingSlot.orderIndex,
    )

    if (existing) {
      updateMutation.mutate(
        { id: existing.id, data: { servings: data.servings, meal_type: data.meal_type } },
        { onSuccess: () => setEditingSlot(null) },
      )
    } else {
      createMutation.mutate(data, {
        onSuccess: () => setEditingSlot(null),
      })
    }
  }

  const handleDelete = () => {
    if (!editingSlot) return

    const dateKey = editingSlot.date
    const dateMeals = mealsByDate.get(dateKey) ?? []
    const existing = dateMeals.find(
      (m) => m.meal_type === editingSlot.mealType && m.order_index === editingSlot.orderIndex,
    )

    if (existing) {
      deleteMutation.mutate(existing.id, {
        onSuccess: () => setEditingSlot(null),
      })
    }
  }

  const handleAddCustomMeal = (dateKey: string) => {
    const dateMeals = mealsByDate.get(dateKey) ?? []
    const maxOrder = dateMeals.reduce((max, m) => Math.max(max, m.order_index), 2)
    setEditingSlot({ date: dateKey, mealType: 'Snack', orderIndex: maxOrder + 1 })
  }

  const weekRangeLabel = `${
    weekDates[0].toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
  } - ${
    weekDates[6].toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })
  }`

  if (mealsLoading) return <div className='p-6 text-gray-500 animate-pulse'>Loading...</div>

  return (
    <div className='p-6'>
      {/* Header */}
      <div className='flex items-center justify-between mb-6'>
        <h1 className='text-2xl font-bold text-gray-900'>Meal Planner</h1>
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
            onClick={goToToday}
            className='px-3 py-1 bg-blue-600 text-white rounded text-sm hover:bg-blue-700'
          >
            Today
          </button>
        </div>
      </div>

      {mealsError && (
        <div className='mb-4 bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
          Failed to load meals: {String(mealsError)}
        </div>
      )}

      {/* Calendar grid */}
      <div className='grid grid-cols-7 gap-2'>
        {weekDates.map((date) => {
          const dateKey = formatDateKey(date)
          const dateMeals = mealsByDate.get(dateKey) ?? []
          const customMeals = dateMeals.filter((m) => m.order_index >= 3)

          return (
            <div
              key={dateKey}
              className={`rounded-lg border p-2 min-h-[200px] ${
                isToday(date)
                  ? 'bg-blue-50 border-blue-200'
                  : 'bg-white border-gray-200'
              }`}
            >
              {/* Day header */}
              <div
                className={`text-center text-sm font-medium mb-2 pb-1 border-b ${
                  isToday(date) ? 'text-blue-700 border-blue-200' : 'text-gray-700 border-gray-100'
                }`}
              >
                {formatDateDisplay(date)}
              </div>

              {/* Default meal slots */}
              <div className='space-y-1'>
                {DEFAULT_MEALS.map(({ type, order }) => {
                  const meal = dateMeals.find(
                    (m) => m.meal_type === type && m.order_index === order,
                  )
                  return (
                    <MealSlot
                      key={type}
                      mealType={type}
                      meal={meal}
                      people={activePeople}
                      recipeNames={recipeNames}
                      onClick={() =>
                        setEditingSlot({ date: dateKey, mealType: type, orderIndex: order })}
                    />
                  )
                })}

                {/* Custom meals */}
                {customMeals.map((meal) => (
                  <MealSlot
                    key={meal.id}
                    mealType={meal.meal_type}
                    meal={meal}
                    people={activePeople}
                    recipeNames={recipeNames}
                    onClick={() =>
                      setEditingSlot({
                        date: dateKey,
                        mealType: meal.meal_type,
                        orderIndex: meal.order_index,
                      })}
                  />
                ))}

                {/* Add custom meal */}
                <button
                  onClick={() => handleAddCustomMeal(dateKey)}
                  className='w-full text-xs text-gray-400 hover:text-blue-600 py-1'
                >
                  + meal
                </button>
              </div>
            </div>
          )
        })}
      </div>

      {/* Meal Editor */}
      {editingSlot && (
        <div className='mt-4'>
          <MealEditor
            date={editingSlot.date}
            mealType={editingSlot.mealType}
            orderIndex={editingSlot.orderIndex}
            existingMeal={(mealsByDate.get(editingSlot.date) ?? []).find(
              (m) =>
                m.meal_type === editingSlot.mealType
                && m.order_index === editingSlot.orderIndex,
            )}
            people={activePeople}
            recipes={recipes}
            onSave={handleSave}
            onCancel={() => setEditingSlot(null)}
            onDelete={(mealsByDate.get(editingSlot.date) ?? []).find(
                (m) =>
                  m.meal_type === editingSlot.mealType
                  && m.order_index === editingSlot.orderIndex,
              )
              ? handleDelete
              : undefined}
          />
          {(createMutation.error || updateMutation.error) && (
            <div className='mt-2 bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
              {String(createMutation.error || updateMutation.error)}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
