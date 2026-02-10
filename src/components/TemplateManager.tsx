import { useEffect, useState } from 'react'
import {
  useDeleteMealTemplate,
  useMealTemplates,
  useUpdateMealTemplate,
} from '../hooks/useMealTemplates'
import { usePeople } from '../hooks/usePeople'
import { useRecipes } from '../hooks/useRecipes'
import type { ParsedMealTemplate } from '../types/mealTemplate'
import { parseMealTemplate } from '../types/mealTemplate'

const MEAL_TYPES = ['Breakfast', 'Lunch', 'Dinner', 'Snack']

export function TemplateManager() {
  const { data: rawTemplates, isLoading, error } = useMealTemplates()
  const { data: people } = usePeople()
  const { data: rawRecipes } = useRecipes()
  const updateMutation = useUpdateMealTemplate()
  const deleteMutation = useDeleteMealTemplate()

  const [search, setSearch] = useState('')
  const [editingId, setEditingId] = useState<string | null>(null)
  const [editName, setEditName] = useState('')
  const [editMealType, setEditMealType] = useState('')
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<string | null>(null)

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (editingId) {
          setEditingId(null)
        } else if (confirmingDeleteId) {
          setConfirmingDeleteId(null)
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [editingId, confirmingDeleteId])

  const recipeNames = new Map(
    rawRecipes?.map((r) => [r.id, r.name]) ?? [],
  )
  const activePeople = people?.filter((p) => p.is_active) ?? []
  const templates: ParsedMealTemplate[] = rawTemplates?.map(parseMealTemplate) ?? []

  const query = search.trim().toLowerCase()
  const filtered = templates.filter((t) => {
    if (!query) return true
    if (t.name.toLowerCase().includes(query)) return true
    if (t.meal_type.toLowerCase().includes(query)) return true
    return t.servings.some((s) => {
      const personName = activePeople.find((p) => p.id === s.person_id)?.name ?? ''
      if (personName.toLowerCase().includes(query)) return true
      if (s.food_type === 'recipe') {
        const recipeName = recipeNames.get(s.recipe_id) ?? ''
        if (recipeName.toLowerCase().includes(query)) return true
      }
      return false
    })
  })

  // Group by meal type
  const groups = new Map<string, ParsedMealTemplate[]>()
  for (const t of filtered) {
    const list = groups.get(t.meal_type) ?? []
    list.push(t)
    groups.set(t.meal_type, list)
  }
  for (const list of groups.values()) {
    list.sort((a, b) => a.name.localeCompare(b.name))
  }
  const groupOrder = [...groups.keys()].sort((a, b) => a.localeCompare(b))

  const startEditing = (template: ParsedMealTemplate) => {
    setEditingId(template.id)
    setEditName(template.name)
    setEditMealType(template.meal_type)
    setConfirmingDeleteId(null)
  }

  const handleSaveEdit = () => {
    if (!editingId || !editName.trim()) return
    updateMutation.mutate(
      { id: editingId, data: { name: editName.trim(), meal_type: editMealType } },
      { onSuccess: () => setEditingId(null) },
    )
  }

  const handleDelete = (id: string) => {
    deleteMutation.mutate(id, {
      onSuccess: () => setConfirmingDeleteId(null),
    })
  }

  const servingSummary = (template: ParsedMealTemplate) => {
    return template.servings.map((s) => {
      const name = activePeople.find((p) => p.id === s.person_id)?.name ?? '?'
      const food = s.food_type === 'recipe'
        ? (recipeNames.get(s.recipe_id) ?? '?')
        : `${s.adhoc_items.length} item${s.adhoc_items.length !== 1 ? 's' : ''}`
      return `${name}: ${food}`
    }).join(', ')
  }

  if (isLoading) return <div className='p-6 text-stone-500 animate-pulse'>Loading...</div>

  if (error) {
    return (
      <div className='p-6'>
        <div className='bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
          Failed to load templates: {String(error)}
        </div>
      </div>
    )
  }

  return (
    <div className='p-6'>
      <div className='flex items-center justify-between mb-6'>
        <h1 className='text-2xl font-bold text-stone-900'>Meal Templates</h1>
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
          className='w-full max-w-md border border-stone-300 rounded px-3 py-2 text-sm mb-4'
        />
      )}

      {templates.length === 0
        ? (
          <div className='text-center py-12 text-stone-500'>
            <p className='text-lg mb-2'>No templates yet</p>
            <p className='text-sm'>
              Save a meal as a template from the Planner tab to get started.
            </p>
          </div>
        )
        : filtered.length === 0
        ? <p className='text-sm text-stone-500'>No templates match &ldquo;{search}&rdquo;</p>
        : (
          <div className='space-y-6'>
            {groupOrder.map((type) => (
              <div key={type}>
                <h2 className='text-sm font-semibold text-stone-500 uppercase tracking-wide mb-2'>
                  {type}
                </h2>
                <div className='space-y-2'>
                  {groups.get(type)!.map((template) => (
                    <div
                      key={template.id}
                      className='bg-white border border-stone-200 rounded-lg p-4'
                    >
                      {editingId === template.id
                        ? (
                          <div className='space-y-2'>
                            <div className='flex gap-2'>
                              <input
                                type='text'
                                value={editName}
                                onChange={(e) => setEditName(e.target.value)}
                                className='border border-stone-300 px-2 py-1 rounded text-sm flex-1'
                                placeholder='Template name'
                                autoFocus
                                onKeyDown={(e) => {
                                  if (e.key === 'Enter') handleSaveEdit()
                                }}
                              />
                              <select
                                value={editMealType}
                                onChange={(e) => setEditMealType(e.target.value)}
                                className='border border-stone-300 px-2 py-1 rounded text-sm'
                              >
                                {MEAL_TYPES.map((mt) => <option key={mt} value={mt}>{mt}</option>)}
                              </select>
                            </div>
                            <div className='text-xs text-stone-500'>
                              {servingSummary(template)}
                            </div>
                            <div className='flex gap-2'>
                              <button
                                onClick={handleSaveEdit}
                                disabled={!editName.trim()}
                                className='bg-primary-600 text-white px-3 py-1 rounded text-sm hover:bg-primary-700 disabled:opacity-50'
                              >
                                Save
                              </button>
                              <button
                                onClick={() => setEditingId(null)}
                                className='border border-stone-300 px-3 py-1 rounded text-sm text-stone-600 hover:bg-stone-50'
                              >
                                Cancel
                              </button>
                            </div>
                            {updateMutation.error && (
                              <div className='bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
                                {String(updateMutation.error)}
                              </div>
                            )}
                          </div>
                        )
                        : (
                          <div className='flex items-center justify-between'>
                            <div>
                              <span className='font-medium'>{template.name}</span>
                              <div className='text-xs text-stone-500 mt-0.5'>
                                {servingSummary(template)}
                              </div>
                            </div>
                            <div className='flex gap-2 items-center'>
                              <button
                                onClick={() => startEditing(template)}
                                className='text-primary-600 text-sm hover:underline'
                              >
                                Edit
                              </button>
                              {confirmingDeleteId === template.id
                                ? (
                                  <span className='flex gap-1 items-center text-sm'>
                                    <span className='text-red-600'>Delete?</span>
                                    <button
                                      onClick={() => handleDelete(template.id)}
                                      className='text-red-700 font-semibold hover:underline'
                                    >
                                      Yes
                                    </button>
                                    <button
                                      onClick={() => setConfirmingDeleteId(null)}
                                      className='text-stone-500 hover:underline'
                                    >
                                      No
                                    </button>
                                  </span>
                                )
                                : (
                                  <button
                                    onClick={() => setConfirmingDeleteId(template.id)}
                                    className='text-red-600 text-sm hover:underline'
                                  >
                                    Delete
                                  </button>
                                )}
                            </div>
                          </div>
                        )}
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}
    </div>
  )
}
