import { useEffect, useState } from 'react'
import {
  useCreateRecipe,
  useDeleteRecipe,
  useImportRecipe,
  usePreviewScaleRecipe,
  useRecipes,
  useToggleFavorite,
  useUpdateRecipe,
} from '../hooks/useRecipes'
import type {
  CreateRecipeDto,
  Ingredient,
  ParsedRecipe,
  ScaleResult,
  TimeValue,
  UpdateRecipeDto,
} from '../types/recipe'
import { formatAmount, formatTime, parseRecipe } from '../types/recipe'
import { IngredientInput } from './IngredientInput'
import { StarRating } from './StarRating'

// --- Sub-components ---

function TagInput({
  label,
  value,
  onChange,
}: {
  label: string
  value: string[]
  onChange: (value: string[]) => void
}) {
  const [input, setInput] = useState('')

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && input.trim()) {
      e.preventDefault()
      onChange([...value, input.trim()])
      setInput('')
    }
  }

  const handleRemove = (index: number) => {
    onChange(value.filter((_, i) => i !== index))
  }

  return (
    <div>
      <label className='block text-sm font-medium text-gray-700 mb-1'>
        {label}
      </label>
      <div className='flex flex-wrap gap-1 mb-1'>
        {value.map((tag, i) => (
          <span
            key={i}
            className='inline-flex items-center bg-gray-100 text-gray-700 text-sm px-2 py-1 rounded'
          >
            {tag}
            <button
              onClick={() => handleRemove(i)}
              className='ml-1 text-gray-400 hover:text-gray-600'
              type='button'
            >
              x
            </button>
          </span>
        ))}
      </div>
      <input
        type='text'
        value={input}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={`Add ${label.toLowerCase()} (press Enter)`}
        className='border border-gray-300 p-2 rounded w-full text-sm'
      />
    </div>
  )
}

function TimeInput({
  label,
  value,
  onChange,
}: {
  label: string
  value: TimeValue | undefined
  onChange: (value: TimeValue | undefined) => void
}) {
  return (
    <div>
      <label className='block text-sm font-medium text-gray-700 mb-1'>
        {label}
      </label>
      <div className='flex gap-2'>
        <input
          type='number'
          min={0}
          value={value?.value ?? ''}
          onChange={(e) => {
            const v = e.target.value
            if (v === '') {
              onChange(undefined)
            } else {
              onChange({ value: parseInt(v), unit: value?.unit ?? 'minutes' })
            }
          }}
          placeholder='0'
          className='border border-gray-300 p-2 rounded w-20 text-sm'
        />
        <select
          value={value?.unit ?? 'minutes'}
          onChange={(e) => {
            if (value) {
              onChange({ ...value, unit: e.target.value as TimeValue['unit'] })
            }
          }}
          className='border border-gray-300 p-2 rounded text-sm'
        >
          <option value='minutes'>minutes</option>
          <option value='hours'>hours</option>
          <option value='days'>days</option>
        </select>
      </div>
    </div>
  )
}

// --- Form data ---

interface RecipeFormData {
  name: string
  description: string
  prep_time?: TimeValue
  cook_time?: TimeValue
  total_time?: TimeValue
  servings: number
  instructions: string
  ingredients: Ingredient[]
  tags: string[]
  notes: string
  icon: string
}

const emptyForm: RecipeFormData = {
  name: '',
  description: '',
  servings: 4,
  instructions: '',
  ingredients: [],
  tags: [],
  notes: '',
  icon: '',
}

// --- Recipe Form ---

function RecipeForm({
  initialData,
  onSubmit,
  onCancel,
  submitLabel,
}: {
  initialData: RecipeFormData
  onSubmit: (data: RecipeFormData) => void
  onCancel: () => void
  submitLabel: string
}) {
  const [form, setForm] = useState<RecipeFormData>(initialData)
  const [validationError, setValidationError] = useState<string | null>(null)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (form.ingredients.length === 0 || form.ingredients.every((i) => !i.name.trim())) {
      setValidationError('At least 1 ingredient with a name required')
      return
    }
    setValidationError(null)
    onSubmit(form)
  }

  return (
    <form onSubmit={handleSubmit} className='space-y-3'>
      <div className='grid grid-cols-1 md:grid-cols-2 gap-3'>
        <div>
          <label className='block text-sm font-medium text-gray-700 mb-1'>
            Name
          </label>
          <input
            type='text'
            required
            value={form.name}
            onChange={(e) => setForm({ ...form, name: e.target.value })}
            className='border border-gray-300 p-2 rounded w-full'
          />
        </div>
        <div>
          <label className='block text-sm font-medium text-gray-700 mb-1'>
            Icon (emoji)
          </label>
          <input
            type='text'
            value={form.icon}
            onChange={(e) => setForm({ ...form, icon: e.target.value })}
            placeholder='e.g. 🍝'
            className='border border-gray-300 p-2 rounded w-24'
          />
        </div>
      </div>

      <div>
        <label className='block text-sm font-medium text-gray-700 mb-1'>
          Description
        </label>
        <textarea
          value={form.description}
          onChange={(e) => setForm({ ...form, description: e.target.value })}
          placeholder='Brief description of the recipe'
          className='border border-gray-300 p-2 rounded w-full'
          rows={2}
        />
      </div>

      <div className='grid grid-cols-2 md:grid-cols-4 gap-3'>
        <TimeInput
          label='Prep time'
          value={form.prep_time}
          onChange={(prep_time) => setForm({ ...form, prep_time })}
        />
        <TimeInput
          label='Cook time'
          value={form.cook_time}
          onChange={(cook_time) => setForm({ ...form, cook_time })}
        />
        <TimeInput
          label='Total time'
          value={form.total_time}
          onChange={(total_time) => setForm({ ...form, total_time })}
        />
        <div>
          <label className='block text-sm font-medium text-gray-700 mb-1'>
            Servings
          </label>
          <input
            type='number'
            min={1}
            required
            value={form.servings}
            onChange={(e) => setForm({ ...form, servings: parseInt(e.target.value) || 1 })}
            className='border border-gray-300 p-2 rounded w-20 text-sm'
          />
        </div>
      </div>

      <IngredientInput
        label='Ingredients'
        value={form.ingredients}
        onChange={(ingredients) => setForm({ ...form, ingredients })}
      />

      <div>
        <label className='block text-sm font-medium text-gray-700 mb-1'>
          Instructions
        </label>
        <textarea
          required
          value={form.instructions}
          onChange={(e) => setForm({ ...form, instructions: e.target.value })}
          className='border border-gray-300 p-2 rounded w-full'
          rows={4}
          placeholder='Step-by-step instructions...'
        />
      </div>

      <TagInput
        label='Tags'
        value={form.tags}
        onChange={(tags) => setForm({ ...form, tags })}
      />

      <div>
        <label className='block text-sm font-medium text-gray-700 mb-1'>
          Notes
        </label>
        <textarea
          value={form.notes}
          onChange={(e) => setForm({ ...form, notes: e.target.value })}
          className='border border-gray-300 p-2 rounded w-full'
          rows={2}
        />
      </div>

      {validationError && (
        <div className='bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
          {validationError}
        </div>
      )}

      <div className='flex gap-2'>
        <button
          type='submit'
          className='bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700'
        >
          {submitLabel}
        </button>
        <button
          type='button'
          onClick={onCancel}
          className='border border-gray-300 px-4 py-2 rounded hover:bg-gray-50'
        >
          Cancel
        </button>
      </div>
    </form>
  )
}

// --- Import Form ---

function ImportRecipeForm({
  onSubmit,
  onCancel,
  error,
}: {
  onSubmit: (markdown: string) => void
  onCancel: () => void
  error?: string
}) {
  const [markdown, setMarkdown] = useState('')

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmit(markdown)
  }

  return (
    <form onSubmit={handleSubmit} className='space-y-3'>
      <div>
        <label className='block text-sm font-medium text-gray-700 mb-1'>
          Paste recipe markdown
        </label>
        <textarea
          required
          value={markdown}
          onChange={(e) => setMarkdown(e.target.value)}
          className='border border-gray-300 p-2 rounded w-full font-mono text-sm'
          rows={12}
          placeholder={`# Recipe Name\nDescription here\nPrep time: 15 min\nServings: 4\n\n## Ingredients\n- 2 cups flour\n- 1 tsp salt\n\n## Instructions\n1. Mix ingredients...\n\n## Tags\ndinner, quick`}
        />
      </div>
      {error && (
        <div className='bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
          {error}
        </div>
      )}
      <div className='flex gap-2'>
        <button
          type='submit'
          className='bg-green-600 text-white px-4 py-2 rounded hover:bg-green-700'
        >
          Import
        </button>
        <button
          type='button'
          onClick={onCancel}
          className='border border-gray-300 px-4 py-2 rounded hover:bg-gray-50'
        >
          Cancel
        </button>
      </div>
    </form>
  )
}

// --- Scale Recipe Panel ---

function ScaleRecipePanel({
  parsed,
  onSaveAsNew,
  onUpdateInPlace,
  onCancel,
  error,
}: {
  parsed: ParsedRecipe
  onSaveAsNew: (ingredients: Ingredient[], servings: number) => void
  onUpdateInPlace: (ingredients: Ingredient[], servings: number) => void
  onCancel: () => void
  error?: string
}) {
  const [targetServings, setTargetServings] = useState(parsed.servings)
  const [preview, setPreview] = useState<ScaleResult | null>(null)
  const [editedIngredients, setEditedIngredients] = useState<Ingredient[] | null>(null)
  const previewMutation = usePreviewScaleRecipe()

  const handlePreview = () => {
    if (targetServings < 1 || targetServings === parsed.servings) return
    setEditedIngredients(null)
    previewMutation.mutate(
      { id: parsed.id, newServings: targetServings },
      {
        onSuccess: (result) => {
          setPreview(result)
          setEditedIngredients(result.ingredients)
        },
      },
    )
  }

  const flaggedIndices = new Set(preview?.flagged.map((f) => f.index) ?? [])

  const handleIngredientChange = (index: number, updated: Ingredient) => {
    if (!editedIngredients) return
    const newList = [...editedIngredients]
    newList[index] = updated
    setEditedIngredients(newList)
  }

  return (
    <div className='border border-purple-200 rounded-lg p-4 bg-purple-50'>
      <h3 className='font-semibold text-lg mb-3'>Scale: {parsed.name}</h3>

      <div className='flex items-center gap-3 mb-4'>
        <span className='text-sm text-gray-600'>
          Current: {parsed.servings} serving{parsed.servings !== 1 ? 's' : ''}
        </span>
        <span className='text-gray-400'>{'\u2192'}</span>
        <input
          type='number'
          min={1}
          value={targetServings}
          onChange={(e) => setTargetServings(parseInt(e.target.value) || 1)}
          className='border border-gray-300 p-1 rounded w-20 text-sm'
        />
        <span className='text-sm text-gray-600'>servings</span>
        <button
          onClick={handlePreview}
          disabled={targetServings < 1 || targetServings === parsed.servings}
          className='bg-purple-600 text-white px-3 py-1 rounded text-sm hover:bg-purple-700 disabled:opacity-50 disabled:cursor-not-allowed'
        >
          Preview
        </button>
      </div>

      {previewMutation.error && (
        <div className='mb-3 bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
          {String(previewMutation.error)}
        </div>
      )}

      {preview && editedIngredients && (
        <>
          {preview.flagged.length > 0 && (
            <div className='mb-3 bg-amber-50 border border-amber-200 rounded p-3 text-amber-800 text-sm'>
              Some ingredients have fractional amounts for discrete units. You can adjust them
              below.
            </div>
          )}

          <div className='space-y-1 mb-4'>
            {editedIngredients.map((ing, i) => (
              <div
                key={i}
                className={`flex gap-2 items-center text-sm ${
                  flaggedIndices.has(i) ? 'bg-amber-50 border border-amber-200 rounded p-1' : 'p-1'
                }`}
              >
                {flaggedIndices.has(i)
                  ? (
                    <input
                      type='number'
                      step='any'
                      value={ing.amount.type === 'single'
                        ? ing.amount.value
                        : (ing.amount as { type: 'range'; min: number; max: number }).min}
                      onChange={(e) => {
                        const val = parseFloat(e.target.value) || 0
                        handleIngredientChange(i, {
                          ...ing,
                          amount: { type: 'single', value: val },
                        })
                      }}
                      className='border border-amber-300 p-1 rounded w-16 text-sm bg-white'
                    />
                  )
                  : <span className='font-medium w-16 text-right'>{formatAmount(ing.amount)}</span>}
                <span className='text-gray-500 w-12'>{ing.unit}</span>
                <span>{ing.name}</span>
                {flaggedIndices.has(i) && (
                  <span className='text-amber-600 text-xs ml-auto'>fractional</span>
                )}
              </div>
            ))}
          </div>

          {error && (
            <div className='mb-3 bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
              {error}
            </div>
          )}

          <div className='flex gap-2'>
            <button
              onClick={() => onSaveAsNew(editedIngredients, targetServings)}
              className='bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700'
            >
              Save as New Recipe
            </button>
            <button
              onClick={() => onUpdateInPlace(editedIngredients, targetServings)}
              className='border border-gray-300 px-4 py-2 rounded text-sm hover:bg-gray-50'
            >
              Update This Recipe
            </button>
            <button
              onClick={onCancel}
              className='border border-gray-300 px-4 py-2 rounded text-sm text-gray-600 hover:bg-gray-50'
            >
              Cancel
            </button>
          </div>
        </>
      )}

      {!preview && (
        <div className='flex gap-2'>
          <button
            onClick={onCancel}
            className='border border-gray-300 px-4 py-2 rounded text-sm text-gray-600 hover:bg-gray-50'
          >
            Cancel
          </button>
        </div>
      )}
    </div>
  )
}

// --- Recipe Detail View ---

function RecipeDetail({
  parsed,
  parentName,
  onEdit,
  onScale,
  onDelete,
  onToggleFavorite,
  onRatingChange,
  onClose,
  confirmingDelete,
  onConfirmDelete,
  onCancelDelete,
}: {
  parsed: ParsedRecipe
  parentName: string | null
  onEdit: () => void
  onScale: () => void
  onDelete: () => void
  onToggleFavorite: () => void
  onRatingChange: (rating: number) => void
  onClose: () => void
  confirmingDelete: boolean
  onConfirmDelete: () => void
  onCancelDelete: () => void
}) {
  return (
    <div className='border border-gray-200 p-6 rounded-lg bg-white md:col-span-2'>
      {/* Header */}
      <div className='flex items-start justify-between mb-4'>
        <div>
          <div className='flex items-center gap-2'>
            {parsed.icon && <span className='text-2xl'>{parsed.icon}</span>}
            <h2 className='text-xl font-bold'>{parsed.name}</h2>
            <button
              onClick={onToggleFavorite}
              className={`text-xl ${
                parsed.is_favorite ? 'text-yellow-500' : 'text-gray-300 hover:text-yellow-400'
              }`}
            >
              {parsed.is_favorite ? '\u2605' : '\u2606'}
            </button>
          </div>
          <div className='flex items-center gap-2 mt-1'>
            <StarRating value={parsed.rating} onChange={onRatingChange} size='md' />
          </div>
          {parsed.description && <p className='text-gray-600 mt-1'>{parsed.description}</p>}
        </div>
        <div className='flex gap-2 items-center'>
          <button
            onClick={onEdit}
            className='text-blue-600 text-sm hover:underline'
          >
            Edit
          </button>
          <button
            onClick={onScale}
            className='text-purple-600 text-sm hover:underline'
          >
            Scale
          </button>
          {confirmingDelete
            ? (
              <span className='flex gap-1 items-center text-sm'>
                <span className='text-red-600'>Delete?</span>
                <button
                  onClick={onConfirmDelete}
                  className='text-red-700 font-semibold hover:underline'
                >
                  Yes
                </button>
                <button
                  onClick={onCancelDelete}
                  className='text-gray-500 hover:underline'
                >
                  No
                </button>
              </span>
            )
            : (
              <button
                onClick={onDelete}
                className='text-red-600 text-sm hover:underline'
              >
                Delete
              </button>
            )}
          <button
            onClick={onClose}
            className='ml-2 text-gray-400 hover:text-gray-600 text-lg'
            title='Back to list'
          >
            {'\u2715'}
          </button>
        </div>
      </div>

      {/* Parent recipe link */}
      {parentName && (
        <div className='text-sm text-gray-500 mb-2'>
          Scaled from: <span className='text-purple-600 font-medium'>{parentName}</span>
        </div>
      )}

      {/* Meta row */}
      <div className='flex flex-wrap gap-4 text-sm text-gray-500 mb-4 pb-4 border-b border-gray-100'>
        <span>Servings: {parsed.servings}</span>
        {parsed.prep_time && <span>Prep: {formatTime(parsed.prep_time)}</span>}
        {parsed.cook_time && <span>Cook: {formatTime(parsed.cook_time)}</span>}
        {parsed.total_time && <span>Total: {formatTime(parsed.total_time)}</span>}
        {parsed.times_made > 0 && (
          <span>Made {parsed.times_made} time{parsed.times_made !== 1 ? 's' : ''}</span>
        )}
      </div>

      <div className='grid grid-cols-1 md:grid-cols-3 gap-6'>
        {/* Ingredients */}
        <div>
          <h3 className='font-semibold mb-2'>Ingredients</h3>
          {parsed.ingredients.length > 0
            ? (
              <ul className='space-y-1'>
                {parsed.ingredients.map((ing, i) => (
                  <li key={i} className='text-sm'>
                    <span className='font-medium'>{formatAmount(ing.amount)}</span>
                    {ing.unit && <span className='text-gray-500'>{` ${ing.unit}`}</span>}
                    <span>{` ${ing.name}`}</span>
                    {ing.notes && <span className='text-gray-400 italic'>{` (${ing.notes})`}</span>}
                  </li>
                ))}
              </ul>
            )
            : <p className='text-sm text-gray-400'>No ingredients listed</p>}
        </div>

        {/* Instructions */}
        <div className='md:col-span-2'>
          <h3 className='font-semibold mb-2'>Instructions</h3>
          {parsed.instructions
            ? (
              <div className='text-sm whitespace-pre-wrap leading-relaxed'>
                {parsed.instructions}
              </div>
            )
            : <p className='text-sm text-gray-400'>No instructions</p>}
        </div>
      </div>

      {/* Nutrition */}
      {parsed.nutrition_per_serving && (
        <div className='mt-4 pt-4 border-t border-gray-100'>
          <h3 className='font-semibold mb-2'>Nutrition (per serving)</h3>
          <div className='flex gap-4 text-sm'>
            {parsed.nutrition_per_serving.calories != null && (
              <span>{parsed.nutrition_per_serving.calories} cal</span>
            )}
            {parsed.nutrition_per_serving.protein_grams != null && (
              <span>{parsed.nutrition_per_serving.protein_grams}g protein</span>
            )}
            {parsed.nutrition_per_serving.carbs_grams != null && (
              <span>{parsed.nutrition_per_serving.carbs_grams}g carbs</span>
            )}
            {parsed.nutrition_per_serving.fat_grams != null && (
              <span>{parsed.nutrition_per_serving.fat_grams}g fat</span>
            )}
          </div>
        </div>
      )}

      {/* Tags */}
      {parsed.tags.length > 0 && (
        <div className='flex flex-wrap gap-1 mt-4 pt-4 border-t border-gray-100'>
          {parsed.tags.map((tag, i) => (
            <span
              key={i}
              className='bg-gray-100 text-gray-600 text-xs px-2 py-0.5 rounded'
            >
              {tag}
            </span>
          ))}
        </div>
      )}

      {/* Notes */}
      {parsed.notes && (
        <div className='mt-4 pt-4 border-t border-gray-100'>
          <h3 className='font-semibold mb-1'>Notes</h3>
          <p className='text-sm text-gray-600 italic'>{parsed.notes}</p>
        </div>
      )}
    </div>
  )
}

// --- Main Component ---

export function RecipeManager() {
  const { data: recipes, isLoading, error } = useRecipes()
  const createMutation = useCreateRecipe()
  const updateMutation = useUpdateRecipe()
  const deleteMutation = useDeleteRecipe()
  const toggleFavoriteMutation = useToggleFavorite()
  const importMutation = useImportRecipe()

  const [searchQuery, setSearchQuery] = useState('')
  const [viewMode, setViewMode] = useState<'list' | 'add' | 'import'>('list')
  const [viewingId, setViewingId] = useState<string | null>(null)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [scalingId, setScalingId] = useState<string | null>(null)
  const [scaleError, setScaleError] = useState<string | null>(null)
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<string | null>(null)

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (scalingId) {
          setScalingId(null)
        } else if (editingId) {
          setEditingId(null)
        } else if (viewingId) {
          setViewingId(null)
          setConfirmingDeleteId(null)
        } else if (viewMode !== 'list') {
          setViewMode('list')
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [scalingId, editingId, viewingId, viewMode])

  const filteredRecipes = recipes?.filter((r) =>
    r.name.toLowerCase().includes(searchQuery.toLowerCase())
  )

  const handleCreate = (formData: RecipeFormData) => {
    const dto: CreateRecipeDto = {
      name: formData.name,
      source: 'manual',
      servings: formData.servings,
      instructions: formData.instructions,
      ingredients: formData.ingredients,
      tags: formData.tags,
      description: formData.description || undefined,
      prep_time: formData.prep_time,
      cook_time: formData.cook_time,
      total_time: formData.total_time,
      notes: formData.notes || undefined,
      icon: formData.icon || undefined,
    }
    createMutation.mutate(dto, {
      onSuccess: () => setViewMode('list'),
    })
  }

  const handleUpdate = (id: string, formData: RecipeFormData) => {
    const dto: UpdateRecipeDto = {
      name: formData.name,
      servings: formData.servings,
      instructions: formData.instructions,
      ingredients: formData.ingredients,
      tags: formData.tags,
      description: formData.description || undefined,
      prep_time: formData.prep_time,
      cook_time: formData.cook_time,
      total_time: formData.total_time,
      notes: formData.notes || undefined,
      icon: formData.icon || undefined,
    }
    updateMutation.mutate({ id, data: dto }, {
      onSuccess: () => setEditingId(null),
    })
  }

  const handleDelete = (id: string) => {
    deleteMutation.mutate(id, {
      onSuccess: () => setConfirmingDeleteId(null),
    })
  }

  const handleImport = (markdown: string) => {
    importMutation.mutate({ markdown }, {
      onSuccess: () => setViewMode('list'),
    })
  }

  const handleScaleSaveAsNew = (
    recipeId: string,
    ingredients: Ingredient[],
    servings: number,
  ) => {
    const source = recipes?.find((r) => r.id === recipeId)
    if (!source) return
    const parsed = parseRecipe(source)
    const dto: CreateRecipeDto = {
      name: `${source.name} (${servings} servings)`,
      source: 'scaled',
      parent_recipe_id: recipeId,
      servings,
      instructions: source.instructions,
      ingredients,
      tags: parsed.tags,
      description: source.description || undefined,
      prep_time: parsed.prep_time ?? undefined,
      cook_time: parsed.cook_time ?? undefined,
      total_time: parsed.total_time ?? undefined,
      notes: source.notes || undefined,
      icon: source.icon || undefined,
    }
    setScaleError(null)
    createMutation.mutate(dto, {
      onSuccess: () => setScalingId(null),
      onError: (err) => setScaleError(String(err)),
    })
  }

  const handleScaleUpdateInPlace = (
    recipeId: string,
    ingredients: Ingredient[],
    servings: number,
  ) => {
    const dto: UpdateRecipeDto = { ingredients, servings }
    setScaleError(null)
    updateMutation.mutate({ id: recipeId, data: dto }, {
      onSuccess: () => setScalingId(null),
      onError: (err) => setScaleError(String(err)),
    })
  }

  if (isLoading) {
    return <div className='p-6 text-gray-500 animate-pulse'>Loading recipes...</div>
  }

  if (error) {
    return (
      <div className='p-6'>
        <div className='bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
          Failed to load recipes: {String(error)}
        </div>
      </div>
    )
  }

  return (
    <div className='p-6'>
      <div className='flex items-center justify-between mb-6'>
        <h1 className='text-2xl font-bold text-gray-900'>Recipes</h1>
        {viewMode === 'list' && (
          <div className='flex gap-2'>
            <button
              onClick={() => setViewMode('add')}
              className='bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700'
            >
              + Add Recipe
            </button>
            <button
              onClick={() => setViewMode('import')}
              className='border border-gray-300 px-4 py-2 rounded hover:bg-gray-50'
            >
              Import Markdown
            </button>
          </div>
        )}
      </div>

      {viewMode === 'add' && (
        <div className='mb-6 border border-gray-200 p-4 rounded-lg bg-white'>
          <h3 className='font-semibold text-lg mb-3'>Add Recipe</h3>
          <RecipeForm
            initialData={emptyForm}
            onSubmit={handleCreate}
            onCancel={() => setViewMode('list')}
            submitLabel='Add Recipe'
          />
          {createMutation.error && (
            <div className='mt-2 bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
              {String(createMutation.error)}
            </div>
          )}
        </div>
      )}

      {viewMode === 'import' && (
        <div className='mb-6 border border-gray-200 p-4 rounded-lg bg-white'>
          <h3 className='font-semibold text-lg mb-3'>Import Recipe from Markdown</h3>
          <ImportRecipeForm
            onSubmit={handleImport}
            onCancel={() => setViewMode('list')}
            error={importMutation.error ? String(importMutation.error) : undefined}
          />
        </div>
      )}

      {viewMode === 'list' && (
        <div className='mb-4'>
          <input
            type='text'
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder='Search recipes...'
            className='border border-gray-300 p-2 rounded w-full md:w-64'
          />
        </div>
      )}

      {filteredRecipes?.length === 0 && viewMode === 'list' && (
        <p className='text-gray-500'>
          {searchQuery
            ? 'No recipes match your search.'
            : 'No recipes yet. Add one to get started!'}
        </p>
      )}

      <div className='grid grid-cols-1 md:grid-cols-2 gap-4'>
        {filteredRecipes?.map((recipe) => {
          const parsed = parseRecipe(recipe)

          if (editingId === recipe.id) {
            return (
              <div
                key={recipe.id}
                className='border border-blue-200 p-4 rounded-lg bg-blue-50 md:col-span-2'
              >
                <h3 className='font-semibold text-lg mb-3'>
                  Edit {recipe.name}
                </h3>
                <RecipeForm
                  initialData={{
                    name: recipe.name,
                    description: recipe.description || '',
                    prep_time: parsed.prep_time ?? undefined,
                    cook_time: parsed.cook_time ?? undefined,
                    total_time: parsed.total_time ?? undefined,
                    servings: recipe.servings,
                    instructions: recipe.instructions,
                    ingredients: parsed.ingredients,
                    tags: parsed.tags,
                    notes: recipe.notes || '',
                    icon: recipe.icon || '',
                  }}
                  onSubmit={(data) => handleUpdate(recipe.id, data)}
                  onCancel={() => setEditingId(null)}
                  submitLabel='Save Changes'
                />
                {updateMutation.error && (
                  <div className='mt-2 bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
                    {String(updateMutation.error)}
                  </div>
                )}
              </div>
            )
          }

          if (scalingId === recipe.id) {
            return (
              <div key={recipe.id} className='md:col-span-2'>
                <ScaleRecipePanel
                  parsed={parsed}
                  onSaveAsNew={(ingredients, servings) =>
                    handleScaleSaveAsNew(recipe.id, ingredients, servings)}
                  onUpdateInPlace={(ingredients, servings) =>
                    handleScaleUpdateInPlace(recipe.id, ingredients, servings)}
                  onCancel={() => {
                    setScalingId(null)
                    setScaleError(null)
                  }}
                  error={scaleError ?? undefined}
                />
              </div>
            )
          }

          if (viewingId === recipe.id) {
            const parentName = parsed.parent_recipe_id
              ? recipes?.find((r) => r.id === parsed.parent_recipe_id)?.name ?? null
              : null
            return (
              <RecipeDetail
                key={recipe.id}
                parsed={parsed}
                parentName={parentName}
                onEdit={() => {
                  setViewingId(null)
                  setEditingId(recipe.id)
                }}
                onScale={() => {
                  setViewingId(null)
                  setScalingId(recipe.id)
                }}
                onDelete={() => setConfirmingDeleteId(recipe.id)}
                onToggleFavorite={() => toggleFavoriteMutation.mutate(recipe.id)}
                onRatingChange={(rating) =>
                  updateMutation.mutate({ id: recipe.id, data: { rating } })}
                onClose={() => {
                  setViewingId(null)
                  setConfirmingDeleteId(null)
                }}
                confirmingDelete={confirmingDeleteId === recipe.id}
                onConfirmDelete={() => {
                  handleDelete(recipe.id)
                  setViewingId(null)
                }}
                onCancelDelete={() => setConfirmingDeleteId(null)}
              />
            )
          }

          return (
            <div
              key={recipe.id}
              className='border border-gray-200 p-4 rounded-lg bg-white'
            >
              <div className='flex items-start justify-between'>
                <div className='flex items-center gap-2'>
                  {recipe.icon && <span className='text-xl'>{recipe.icon}</span>}
                  <button
                    onClick={() => setViewingId(recipe.id)}
                    className='font-semibold text-lg hover:text-blue-600 text-left'
                  >
                    {recipe.name}
                  </button>
                  <button
                    onClick={() => toggleFavoriteMutation.mutate(recipe.id)}
                    className={`text-lg ${
                      recipe.is_favorite ? 'text-yellow-500' : 'text-gray-300 hover:text-yellow-400'
                    }`}
                    title={recipe.is_favorite ? 'Unfavorite' : 'Favorite'}
                  >
                    {recipe.is_favorite ? '\u2605' : '\u2606'}
                  </button>
                  <StarRating value={recipe.rating} size='sm' />
                </div>
              </div>

              {recipe.description && (
                <p className='text-sm text-gray-600 mt-1'>{recipe.description}</p>
              )}

              <div className='flex gap-4 mt-2 text-sm text-gray-500'>
                <span>Servings: {recipe.servings}</span>
                {parsed.prep_time && <span>Prep: {formatTime(parsed.prep_time)}</span>}
                {parsed.cook_time && <span>Cook: {formatTime(parsed.cook_time)}</span>}
                {parsed.total_time && <span>Total: {formatTime(parsed.total_time)}</span>}
              </div>

              {parsed.tags.length > 0 && (
                <div className='flex flex-wrap gap-1 mt-2'>
                  {parsed.tags.map((tag, i) => (
                    <span
                      key={i}
                      className='bg-gray-100 text-gray-600 text-xs px-2 py-0.5 rounded'
                    >
                      {tag}
                    </span>
                  ))}
                </div>
              )}

              {recipe.times_made > 0 && (
                <p className='text-xs text-gray-400 mt-2'>
                  Made {recipe.times_made} time{recipe.times_made !== 1 ? 's' : ''}
                </p>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}
