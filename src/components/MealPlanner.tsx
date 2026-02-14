import { useEffect, useMemo, useState } from 'react'
import {
  useCreateMeal,
  useDeleteMeal,
  useMealsForDateRange,
  useUpdateMeal,
} from '../hooks/useMeals'
import {
  useCreateTemplateFromMeal,
  useDeleteMealTemplate,
  useMealTemplates,
} from '../hooks/useMealTemplates'
import { usePeople } from '../hooks/usePeople'
import { useRecipes } from '../hooks/useRecipes'
import type { CreateMealDto, ParsedMeal, PersonServing } from '../types/meal'
import { parseMeal } from '../types/meal'
import type { ParsedMealTemplate } from '../types/mealTemplate'
import { parseMealTemplate } from '../types/mealTemplate'
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
import { IconArrowLeft, IconArrowRight, IconClose, IconPlus, IconWarning } from './Icon'
import { IngredientInput } from './IngredientInput'
import { ServingMismatchBanner } from './ServingMismatchBanner'
import { SuggestionPanel } from './SuggestionPanel'
import { useToast } from './Toast'

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
  recipes: { id: string; name: string; servings: number }[]
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
    <div className='border border-stone-200 rounded p-3 bg-stone-50'>
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
                  ? 'bg-primary-50 border-primary-300 text-primary-700'
                  : 'bg-white border-stone-300 text-stone-500 hover:border-primary-300'
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
            className='input-sm flex-1'
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
            className='input-sm w-16'
            title='Servings count'
          />
          <span className='text-xs text-stone-500'>servings</span>
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
          ? 'bg-white border-stone-200 hover:border-primary-300'
          : 'bg-stone-50 border-dashed border-stone-300 hover:border-primary-300 hover:bg-primary-50'
      }`}
    >
      <div className='font-medium text-stone-700 mb-1'>{mealType}</div>
      {hasServings
        ? (
          <div className='space-y-0.5'>
            {meal.servings.map((s, i) => {
              const personName = people.find((p) => p.id === s.person_id)?.name ?? '?'
              const food = s.food_type === 'recipe'
                ? (recipeNames.get(s.recipe_id) ?? 'Unknown recipe')
                : `${s.adhoc_items.length} item${s.adhoc_items.length !== 1 ? 's' : ''}`
              return (
                <div key={i} className='text-stone-500 truncate'>
                  <span className='text-stone-700'>{personName}</span>: {food}
                </div>
              )
            })}
          </div>
        )
        : <div className='text-stone-400'>+ Plan</div>}
    </button>
  )
}

// --- TemplatePicker ---

function TemplateRow({
  template,
  people,
  recipeNames,
  confirmingDelete,
  onApply,
  onRequestDelete,
  onConfirmDelete,
  onCancelDelete,
}: {
  template: ParsedMealTemplate
  people: Person[]
  recipeNames: Map<string, string>
  confirmingDelete: boolean
  onApply: (template: ParsedMealTemplate) => void
  onRequestDelete: () => void
  onConfirmDelete: () => void
  onCancelDelete: () => void
}) {
  return (
    <div className='flex items-center justify-between bg-white border border-stone-200 rounded p-2'>
      <button
        onClick={() => onApply(template)}
        className='flex-1 text-left text-sm hover:text-primary-700'
      >
        <span className='font-medium'>{template.name}</span>
        <div className='text-xs text-stone-500 mt-0.5'>
          {template.servings.map((s) => {
            const name = people.find((p) => p.id === s.person_id)?.name ?? '?'
            const food = s.food_type === 'recipe'
              ? (recipeNames.get(s.recipe_id) ?? '?')
              : `${s.adhoc_items.length} items`
            return `${name}: ${food}`
          }).join(', ')}
        </div>
      </button>
      {confirmingDelete
        ? (
          <span className='flex gap-1 items-center text-sm ml-2'>
            <span className='text-red-600'>Delete?</span>
            <button
              onClick={onConfirmDelete}
              className='text-red-700 font-semibold hover:underline'
            >
              Yes
            </button>
            <button
              onClick={onCancelDelete}
              className='text-stone-500 hover:underline'
            >
              No
            </button>
          </span>
        )
        : (
          <button
            onClick={onRequestDelete}
            className='text-red-400 hover:text-red-600 text-xs ml-2 px-1'
            title='Delete template'
          >
            <IconClose className='w-3 h-3' />
          </button>
        )}
    </div>
  )
}

function TemplatePicker({
  templates,
  mealType,
  recipeNames,
  people,
  onApply,
  onDelete,
  onClose,
}: {
  templates: ParsedMealTemplate[]
  mealType: string
  recipeNames: Map<string, string>
  people: Person[]
  onApply: (template: ParsedMealTemplate) => void
  onDelete: (id: string) => void
  onClose: () => void
}) {
  const [search, setSearch] = useState('')
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<string | null>(null)
  const query = search.trim().toLowerCase()

  // Filter by search term (match name, meal type, or person/food names)
  const filtered = templates.filter((t) => {
    if (!query) return true
    if (t.name.toLowerCase().includes(query)) return true
    if (t.meal_type.toLowerCase().includes(query)) return true
    return t.servings.some((s) => {
      const personName = people.find((p) => p.id === s.person_id)?.name ?? ''
      if (personName.toLowerCase().includes(query)) return true
      if (s.food_type === 'recipe') {
        const recipeName = recipeNames.get(s.recipe_id) ?? ''
        if (recipeName.toLowerCase().includes(query)) return true
      }
      return false
    })
  })

  // Group by meal type, current meal type first
  const groups = new Map<string, ParsedMealTemplate[]>()
  for (const t of filtered) {
    const list = groups.get(t.meal_type) ?? []
    list.push(t)
    groups.set(t.meal_type, list)
  }
  // Sort each group alphabetically by name
  for (const list of groups.values()) {
    list.sort((a, b) => a.name.localeCompare(b.name))
  }
  // Order groups: current meal type first, then alphabetical
  const groupOrder = [...groups.keys()].sort((a, b) => {
    if (a === mealType && b !== mealType) return -1
    if (a !== mealType && b === mealType) return 1
    return a.localeCompare(b)
  })

  return (
    <div className='border border-primary-200 rounded-lg p-3 bg-primary-50 mb-3 animate-slide-down'>
      <div className='flex items-center justify-between mb-2'>
        <h4 className='font-medium text-sm text-primary-800'>Choose a Template</h4>
        <button onClick={onClose} className='text-stone-400 hover:text-stone-600 text-sm'>
          <IconClose className='w-3.5 h-3.5' />
        </button>
      </div>

      {templates.length > 0 && (
        <input
          type='text'
          value={search}
          onChange={(e) => {
            setSearch(e.target.value)
            setConfirmingDeleteId(null)
          }}
          placeholder='Search templates...'
          className='input-sm w-full mb-2'
          autoFocus
        />
      )}

      {templates.length === 0
        ? <p className='text-sm text-stone-500'>No templates saved yet.</p>
        : filtered.length === 0
        ? <p className='text-sm text-stone-500'>No templates match &ldquo;{search}&rdquo;</p>
        : (
          <div className='space-y-3 max-h-64 overflow-y-auto'>
            {groupOrder.map((type) => (
              <div key={type}>
                <div className='text-xs font-semibold text-stone-500 uppercase tracking-wide mb-1'>
                  {type}
                </div>
                <div className='space-y-1'>
                  {groups.get(type)!.map((t) => (
                    <TemplateRow
                      key={t.id}
                      template={t}
                      people={people}
                      recipeNames={recipeNames}
                      confirmingDelete={confirmingDeleteId === t.id}
                      onApply={onApply}
                      onRequestDelete={() => setConfirmingDeleteId(t.id)}
                      onConfirmDelete={() => {
                        onDelete(t.id)
                        setConfirmingDeleteId(null)
                      }}
                      onCancelDelete={() => setConfirmingDeleteId(null)}
                    />
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}
    </div>
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
  templates,
  recipeNames,
  onSave,
  onCancel,
  onDelete,
  onSaveAsTemplate,
  onDeleteTemplate,
}: {
  date: string
  mealType: string
  orderIndex: number
  existingMeal: ParsedMeal | undefined
  people: Person[]
  recipes: { id: string; name: string; servings: number }[]
  templates: ParsedMealTemplate[]
  recipeNames: Map<string, string>
  onSave: (data: CreateMealDto) => void
  onCancel: () => void
  onDelete?: () => void
  onSaveAsTemplate?: (mealId: string, name: string) => void
  onDeleteTemplate: (id: string) => void
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
  const [showTemplatePicker, setShowTemplatePicker] = useState(false)
  const [templateName, setTemplateName] = useState('')
  const [showSaveTemplate, setShowSaveTemplate] = useState(false)
  const [showSuggestionPanel, setShowSuggestionPanel] = useState(false)
  const isCustom = orderIndex >= 3
  const [dismissedMismatches, setDismissedMismatches] = useState<Set<string>>(new Set())

  // Detect serving mismatches: recipe makes X servings but planned total < X
  const servingMismatches = useMemo(() => {
    const recipeGroups = new Map<
      string,
      { totalPlanned: number; recipeName: string; recipeServings: number; personIds: string[] }
    >()

    for (const [personId, serving] of servingsMap) {
      if (serving.food_type !== 'recipe') continue
      const recipe = recipes.find((r) => r.id === serving.recipe_id)
      if (!recipe) continue

      const existing = recipeGroups.get(serving.recipe_id)
      if (existing) {
        existing.totalPlanned += serving.servings_count
        existing.personIds.push(personId)
      } else {
        recipeGroups.set(serving.recipe_id, {
          totalPlanned: serving.servings_count,
          recipeName: recipe.name,
          recipeServings: recipe.servings,
          personIds: [personId],
        })
      }
    }

    return [...recipeGroups.entries()]
      .filter(([, info]) => info.totalPlanned < info.recipeServings - 0.01)
      .map(([recipeId, info]) => ({ recipeId, ...info }))
  }, [servingsMap, recipes])

  const handleAdjustServings = (recipeId: string) => {
    const mismatch = servingMismatches.find((m) => m.recipeId === recipeId)
    if (!mismatch) return

    const perPerson = mismatch.recipeServings / mismatch.personIds.length
    const newMap = new Map(servingsMap)
    for (const [personId, serving] of newMap) {
      if (serving.food_type === 'recipe' && serving.recipe_id === recipeId) {
        newMap.set(personId, { ...serving, servings_count: perPerson })
      }
    }
    setServingsMap(newMap)
  }

  const handleDismissMismatch = (recipeId: string) => {
    setDismissedMismatches((prev) => new Set(prev).add(recipeId))
  }

  const handleApplyTemplate = (template: ParsedMealTemplate) => {
    const newMap = new Map(servingsMap)
    // Only fill in people defined in the template; leave others untouched
    for (const s of template.servings) {
      newMap.set(s.person_id, s)
    }
    setServingsMap(newMap)
    setShowTemplatePicker(false)
  }

  const handleApplySuggestion = (recipeId: string, personIds: string[]) => {
    const newMap = new Map(servingsMap)
    for (const personId of personIds) {
      newMap.set(personId, {
        food_type: 'recipe',
        person_id: personId,
        recipe_id: recipeId,
        servings_count: 1,
        notes: null,
      })
    }
    setServingsMap(newMap)
    setShowSuggestionPanel(false)
  }

  const handleSaveAsTemplate = () => {
    if (!existingMeal || !templateName.trim()) return
    onSaveAsTemplate?.(existingMeal.id, templateName.trim())
    setShowSaveTemplate(false)
    setTemplateName('')
  }

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

    // Filter out empty-name ingredients from ad-hoc servings
    const servings: PersonServing[] = []
    for (const serving of servingsMap.values()) {
      if (serving.food_type === 'adhoc') {
        const validItems = serving.adhoc_items.filter((item) => item.name.trim() !== '')
        if (validItems.length === 0) {
          setValidationError('Ad-hoc items must have at least one ingredient with a name')
          return
        }
        servings.push({ ...serving, adhoc_items: validItems })
      } else {
        servings.push(serving)
      }
    }

    setValidationError(null)
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
    <div className='card p-4 border-primary-200 animate-slide-up'>
      <div className='flex items-center justify-between mb-4'>
        <div>
          <h3 className='font-semibold text-lg'>
            {isCustom
              ? (
                <input
                  type='text'
                  value={customMealType}
                  onChange={(e) => setCustomMealType(e.target.value)}
                  className='input text-lg font-semibold'
                  placeholder='Meal name (e.g. Snack)'
                />
              )
              : mealType}
          </h3>
          <p className='text-sm text-stone-500'>{dateLabel}</p>
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
                    className='text-stone-500 hover:underline'
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

      {showTemplatePicker && (
        <TemplatePicker
          templates={templates}
          mealType={isCustom ? customMealType : mealType}
          recipeNames={recipeNames}
          people={people}
          onApply={handleApplyTemplate}
          onDelete={onDeleteTemplate}
          onClose={() => setShowTemplatePicker(false)}
        />
      )}

      {showSuggestionPanel && (
        <SuggestionPanel
          people={people}
          date={date}
          mealType={isCustom ? customMealType : mealType}
          onApply={handleApplySuggestion}
          onClose={() => setShowSuggestionPanel(false)}
        />
      )}

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

      {servingMismatches
        .filter((m) => !dismissedMismatches.has(m.recipeId))
        .map((m) => (
          <div key={m.recipeId} className='mb-2'>
            <ServingMismatchBanner
              recipeName={m.recipeName}
              recipeServings={m.recipeServings}
              totalPlanned={m.totalPlanned}
              numPeople={m.personIds.length}
              onAdjust={() => handleAdjustServings(m.recipeId)}
              onDismiss={() => handleDismissMismatch(m.recipeId)}
            />
          </div>
        ))}

      {validationError && (
        <div className='mb-2 panel-error text-red-700 text-sm'>
          {validationError}
        </div>
      )}

      {showSaveTemplate && (
        <div className='mb-2 flex gap-2 items-center'>
          <input
            type='text'
            value={templateName}
            onChange={(e) => setTemplateName(e.target.value)}
            placeholder='Template name...'
            className='input-sm flex-1'
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleSaveAsTemplate()
            }}
          />
          <button
            onClick={handleSaveAsTemplate}
            disabled={!templateName.trim()}
            className='btn-sm btn-primary'
          >
            Save
          </button>
          <button
            onClick={() => setShowSaveTemplate(false)}
            className='text-stone-500 text-sm hover:underline'
          >
            Cancel
          </button>
        </div>
      )}

      <div className='flex gap-2'>
        <button
          onClick={handleSave}
          className='btn-sm btn-primary'
        >
          {existingMeal ? 'Save Changes' : 'Create Meal'}
        </button>
        <button
          onClick={() => {
            setShowTemplatePicker(!showTemplatePicker)
            setShowSuggestionPanel(false)
          }}
          className='btn-sm btn-outline border-primary-300 text-primary-700 hover:bg-primary-50'
        >
          Use Template
        </button>
        <button
          onClick={() => {
            setShowSuggestionPanel(!showSuggestionPanel)
            setShowTemplatePicker(false)
          }}
          className='btn-sm btn-outline border-secondary-300 text-secondary-700 hover:bg-secondary-50'
        >
          Suggest
        </button>
        {existingMeal && onSaveAsTemplate && (
          <button
            onClick={() => setShowSaveTemplate(!showSaveTemplate)}
            className='btn-sm btn-outline'
          >
            Save as Template
          </button>
        )}
        <button
          onClick={onCancel}
          className='btn-sm btn-outline'
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
  const { toast } = useToast()
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

  const { data: rawTemplates } = useMealTemplates()
  const createTemplateMutation = useCreateTemplateFromMeal()
  const deleteTemplateMutation = useDeleteMealTemplate()

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
  const recipes = useMemo(
    () => rawRecipes?.map((r) => ({ id: r.id, name: r.name, servings: r.servings })) ?? [],
    [rawRecipes],
  )
  const recipeNames = new Map(recipes.map((r) => [r.id, r.name]))

  // Parse templates
  const templates: ParsedMealTemplate[] = rawTemplates?.map(parseMealTemplate) ?? []

  // Parse and group meals by date
  const parsedMeals = useMemo(() => meals?.map(parseMeal) ?? [], [meals])
  const mealsByDate = new Map<string, ParsedMeal[]>()
  for (const m of parsedMeals) {
    const key = m.date
    if (!mealsByDate.has(key)) {
      mealsByDate.set(key, [])
    }
    mealsByDate.get(key)!.push(m)
  }

  const activePeople = people?.filter((p) => p.is_active) ?? []

  // Compute serving mismatches across all meals this week
  const weeklyMismatches = useMemo(() => {
    const mismatches: {
      date: string
      mealType: string
      recipeName: string
      recipeServings: number
      totalPlanned: number
    }[] = []

    for (const meal of parsedMeals) {
      // Group recipe servings by recipe_id within this meal
      const recipeGroups = new Map<
        string,
        { totalPlanned: number; recipeServings: number; recipeName: string }
      >()

      for (const s of meal.servings) {
        if (s.food_type !== 'recipe') continue
        const recipe = recipes.find((r) => r.id === s.recipe_id)
        if (!recipe) continue

        const existing = recipeGroups.get(s.recipe_id)
        if (existing) {
          existing.totalPlanned += s.servings_count
        } else {
          recipeGroups.set(s.recipe_id, {
            totalPlanned: s.servings_count,
            recipeName: recipe.name,
            recipeServings: recipe.servings,
          })
        }
      }

      for (const info of recipeGroups.values()) {
        if (info.totalPlanned < info.recipeServings - 0.01) {
          const dateObj = new Date(meal.date + 'T00:00:00')
          const dayLabel = dateObj.toLocaleDateString('en-US', {
            weekday: 'short',
          })
          mismatches.push({
            date: `${dayLabel} ${meal.meal_type}`,
            mealType: meal.meal_type,
            recipeName: info.recipeName,
            recipeServings: info.recipeServings,
            totalPlanned: info.totalPlanned,
          })
        }
      }
    }

    return mismatches
  }, [parsedMeals, recipes])

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
        {
          onSuccess: () => {
            setEditingSlot(null)
            toast('Meal saved')
          },
        },
      )
    } else {
      createMutation.mutate(data, {
        onSuccess: () => {
          setEditingSlot(null)
          toast('Meal saved')
        },
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
        onSuccess: () => {
          setEditingSlot(null)
          toast('Meal deleted')
        },
      })
    }
  }

  const handleAddCustomMeal = (dateKey: string) => {
    const dateMeals = mealsByDate.get(dateKey) ?? []
    const maxOrder = dateMeals.reduce((max, m) => Math.max(max, m.order_index), 2)
    setEditingSlot({ date: dateKey, mealType: 'Snack', orderIndex: maxOrder + 1 })
  }

  const handleSaveAsTemplate = (mealId: string, name: string) => {
    createTemplateMutation.mutate({ meal_id: mealId, name }, {
      onSuccess: () => toast('Template saved'),
    })
  }

  const handleDeleteTemplate = (id: string) => {
    deleteTemplateMutation.mutate(id)
  }

  const weekRangeLabel = `${
    weekDates[0].toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
  } - ${
    weekDates[6].toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })
  }`

  if (mealsLoading) return <div className='p-6 text-stone-500 animate-pulse'>Loading...</div>

  return (
    <div className='p-6'>
      {/* Header */}
      <div className='flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3 mb-6'>
        <h1 className='text-2xl font-bold text-stone-900'>Meal Planner</h1>
        <div className='flex items-center gap-3'>
          <button
            onClick={prevWeek}
            className='btn-sm btn-outline'
          >
            <IconArrowLeft className='w-4 h-4 mr-1' /> Prev
          </button>
          <span className='text-sm font-medium text-stone-700 text-center whitespace-nowrap'>
            {weekRangeLabel}
          </span>
          <button
            onClick={nextWeek}
            className='btn-sm btn-outline'
          >
            Next <IconArrowRight className='w-4 h-4 ml-1' />
          </button>
          <button
            onClick={goToToday}
            className='btn-sm btn-primary'
          >
            Today
          </button>
        </div>
      </div>

      {mealsError && (
        <div className='mb-4 panel-error text-red-700 text-sm'>
          Failed to load meals: {String(mealsError)}
        </div>
      )}

      {/* Calendar grid */}
      <div className='grid grid-cols-2 md:grid-cols-4 lg:grid-cols-7 gap-2'>
        {weekDates.map((date) => {
          const dateKey = formatDateKey(date)
          const dateMeals = mealsByDate.get(dateKey) ?? []
          const customMeals = dateMeals.filter((m) => m.order_index >= 3)

          return (
            <div
              key={dateKey}
              className={`rounded-lg border p-2 min-h-[140px] lg:min-h-[200px] ${
                isToday(date)
                  ? 'bg-primary-50 border-primary-200'
                  : 'bg-white border-stone-200'
              }`}
            >
              {/* Day header */}
              <div
                className={`text-center text-sm font-medium mb-2 pb-1 border-b ${
                  isToday(date)
                    ? 'text-primary-700 border-primary-200'
                    : 'text-stone-700 border-stone-100'
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
                  className='w-full text-xs text-stone-400 hover:text-primary-600 py-1'
                >
                  <IconPlus className='w-3 h-3 inline' /> meal
                </button>
              </div>
            </div>
          )
        })}
      </div>

      {/* Serving mismatch warnings */}
      {weeklyMismatches.length > 0 && (
        <div className='mt-3 panel-warning animate-slide-down'>
          <div className='flex items-center gap-2 text-sm text-amber-800'>
            <IconWarning className='w-4 h-4 text-amber-600' />
            <span className='font-medium'>
              {weeklyMismatches.length} partial recipe{weeklyMismatches.length !== 1 ? 's' : ''}
            </span>
            <span className='text-amber-600'>
              {'\u2014'} shopping list amounts are less than full recipes
            </span>
          </div>
          <div className='mt-2 space-y-1'>
            {weeklyMismatches.map((m, i) => (
              <div key={i} className='text-xs text-amber-700 ml-6'>
                <span className='text-amber-500'>{m.date}:</span>{' '}
                <span className='font-medium'>{m.recipeName}</span> ({m.totalPlanned} of{' '}
                {m.recipeServings} servings planned)
              </div>
            ))}
          </div>
        </div>
      )}

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
            templates={templates}
            recipeNames={recipeNames}
            onSave={handleSave}
            onCancel={() => setEditingSlot(null)}
            onDelete={(mealsByDate.get(editingSlot.date) ?? []).find(
                (m) =>
                  m.meal_type === editingSlot.mealType
                  && m.order_index === editingSlot.orderIndex,
              )
              ? handleDelete
              : undefined}
            onSaveAsTemplate={handleSaveAsTemplate}
            onDeleteTemplate={handleDeleteTemplate}
          />
          {(createMutation.error || updateMutation.error) && (
            <div className='mt-2 panel-error text-red-700 text-sm'>
              {String(createMutation.error || updateMutation.error)}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
