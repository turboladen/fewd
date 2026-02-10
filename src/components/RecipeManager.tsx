import { open } from '@tauri-apps/plugin-dialog'
import { useEffect, useState } from 'react'
import {
  useCreateRecipe,
  useDeleteRecipe,
  useEnhanceInstructions,
  useImportRecipe,
  useImportRecipeFromFile,
  useImportRecipeFromUrl,
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
import { AdaptRecipePanel } from './AdaptRecipePanel'
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
      <label className='block text-sm font-medium text-stone-700 mb-1'>
        {label}
      </label>
      <div className='flex flex-wrap gap-1 mb-1'>
        {value.map((tag, i) => (
          <span
            key={i}
            className='inline-flex items-center bg-stone-100 text-stone-700 text-sm px-2 py-1 rounded'
          >
            {tag}
            <button
              onClick={() => handleRemove(i)}
              className='ml-1 text-stone-400 hover:text-stone-600'
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
        className='border border-stone-300 p-2 rounded w-full text-sm'
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
      <label className='block text-sm font-medium text-stone-700 mb-1'>
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
          className='border border-stone-300 p-2 rounded w-20 text-sm'
        />
        <select
          value={value?.unit ?? 'minutes'}
          onChange={(e) => {
            if (value) {
              onChange({ ...value, unit: e.target.value as TimeValue['unit'] })
            }
          }}
          className='border border-stone-300 p-2 rounded text-sm'
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
          <label className='block text-sm font-medium text-stone-700 mb-1'>
            Name
          </label>
          <input
            type='text'
            required
            value={form.name}
            onChange={(e) => setForm({ ...form, name: e.target.value })}
            className='border border-stone-300 p-2 rounded w-full'
          />
        </div>
        <div>
          <label className='block text-sm font-medium text-stone-700 mb-1'>
            Icon (emoji)
          </label>
          <input
            type='text'
            value={form.icon}
            onChange={(e) => setForm({ ...form, icon: e.target.value })}
            placeholder='e.g. 🍝'
            className='border border-stone-300 p-2 rounded w-24'
          />
        </div>
      </div>

      <div>
        <label className='block text-sm font-medium text-stone-700 mb-1'>
          Description
        </label>
        <textarea
          value={form.description}
          onChange={(e) => setForm({ ...form, description: e.target.value })}
          placeholder='Brief description of the recipe'
          className='border border-stone-300 p-2 rounded w-full'
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
          <label className='block text-sm font-medium text-stone-700 mb-1'>
            Servings
          </label>
          <input
            type='number'
            min={1}
            required
            value={form.servings}
            onChange={(e) => setForm({ ...form, servings: parseInt(e.target.value) || 1 })}
            className='border border-stone-300 p-2 rounded w-20 text-sm'
          />
        </div>
      </div>

      <IngredientInput
        label='Ingredients'
        value={form.ingredients}
        onChange={(ingredients) => setForm({ ...form, ingredients })}
      />

      <div>
        <label className='block text-sm font-medium text-stone-700 mb-1'>
          Instructions
        </label>
        <textarea
          required
          value={form.instructions}
          onChange={(e) => setForm({ ...form, instructions: e.target.value })}
          className='border border-stone-300 p-2 rounded w-full'
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
        <label className='block text-sm font-medium text-stone-700 mb-1'>
          Notes
        </label>
        <textarea
          value={form.notes}
          onChange={(e) => setForm({ ...form, notes: e.target.value })}
          className='border border-stone-300 p-2 rounded w-full'
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
          className='bg-primary-600 text-white px-4 py-2 rounded hover:bg-primary-700'
        >
          {submitLabel}
        </button>
        <button
          type='button'
          onClick={onCancel}
          className='border border-stone-300 px-4 py-2 rounded hover:bg-stone-50'
        >
          Cancel
        </button>
      </div>
    </form>
  )
}

// --- Import Form ---

function ImportRecipeForm({
  onSubmitMarkdown,
  onSubmitUrl,
  onSubmitFile,
  onCancel,
  markdownError,
  urlError,
  fileError,
  urlLoading,
  fileLoading,
}: {
  onSubmitMarkdown: (markdown: string) => void
  onSubmitUrl: (url: string) => void
  onSubmitFile: (filePath: string) => void
  onCancel: () => void
  markdownError?: string
  urlError?: string
  fileError?: string
  urlLoading?: boolean
  fileLoading?: boolean
}) {
  const [importMode, setImportMode] = useState<'markdown' | 'url' | 'pdf'>('url')
  const [markdown, setMarkdown] = useState('')
  const [url, setUrl] = useState('')
  const [selectedFile, setSelectedFile] = useState<string | null>(null)

  const handleMarkdownSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmitMarkdown(markdown)
  }

  const handleUrlSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmitUrl(url)
  }

  const handleChooseFile = async () => {
    const result = await open({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    })
    if (result) {
      setSelectedFile(result)
    }
  }

  const handleFileSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (selectedFile) {
      onSubmitFile(selectedFile)
    }
  }

  const tabs: { key: typeof importMode; label: string }[] = [
    { key: 'url', label: 'From URL' },
    { key: 'pdf', label: 'From PDF' },
    { key: 'markdown', label: 'Paste Markdown' },
  ]

  return (
    <div className='space-y-3'>
      <div className='flex gap-1 border-b border-stone-200'>
        {tabs.map((tab) => (
          <button
            key={tab.key}
            onClick={() => setImportMode(tab.key)}
            className={`px-3 py-2 text-sm font-medium border-b-2 -mb-px ${
              importMode === tab.key
                ? 'border-primary-600 text-primary-600'
                : 'border-transparent text-stone-500 hover:text-stone-700'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {importMode === 'markdown' && (
        <form onSubmit={handleMarkdownSubmit} className='space-y-3'>
          <div>
            <label className='block text-sm font-medium text-stone-700 mb-1'>
              Paste recipe markdown
            </label>
            <textarea
              required
              value={markdown}
              onChange={(e) => setMarkdown(e.target.value)}
              className='border border-stone-300 p-2 rounded w-full font-mono text-sm'
              rows={12}
              placeholder={`# Recipe Name\nDescription here\nPrep time: 15 min\nServings: 4\n\n## Ingredients\n- 2 cups flour\n- 1 tsp salt\n\n## Instructions\n1. Mix ingredients...\n\n## Tags\ndinner, quick`}
            />
          </div>
          {markdownError && (
            <div className='bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
              {markdownError}
            </div>
          )}
          <div className='flex gap-2'>
            <button
              type='submit'
              className='bg-primary-600 text-white px-4 py-2 rounded hover:bg-primary-700'
            >
              Import
            </button>
            <button
              type='button'
              onClick={onCancel}
              className='border border-stone-300 px-4 py-2 rounded hover:bg-stone-50'
            >
              Cancel
            </button>
          </div>
        </form>
      )}

      {importMode === 'url' && (
        <form onSubmit={handleUrlSubmit} className='space-y-3'>
          <div>
            <label className='block text-sm font-medium text-stone-700 mb-1'>
              Recipe URL
            </label>
            <input
              type='url'
              required
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder='https://example.com/recipe/...'
              className='border border-stone-300 p-2 rounded w-full text-sm'
            />
            <p className='text-xs text-stone-500 mt-1'>
              Paste a link to a recipe page. AI will extract the recipe automatically.
            </p>
          </div>
          {urlError && (
            <div className='bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
              {urlError}
            </div>
          )}
          <div className='flex gap-2'>
            <button
              type='submit'
              disabled={urlLoading}
              className='bg-primary-600 text-white px-4 py-2 rounded hover:bg-primary-700 disabled:opacity-50 disabled:cursor-wait'
            >
              {urlLoading ? 'Analyzing recipe...' : 'Import'}
            </button>
            <button
              type='button'
              onClick={onCancel}
              disabled={urlLoading}
              className='border border-stone-300 px-4 py-2 rounded hover:bg-stone-50 disabled:opacity-50'
            >
              Cancel
            </button>
          </div>
        </form>
      )}

      {importMode === 'pdf' && (
        <form onSubmit={handleFileSubmit} className='space-y-3'>
          <div>
            <label className='block text-sm font-medium text-stone-700 mb-1'>
              PDF File
            </label>
            <div className='flex gap-2 items-center'>
              <button
                type='button'
                onClick={handleChooseFile}
                className='border border-stone-300 px-3 py-2 rounded text-sm hover:bg-stone-50'
              >
                Choose File
              </button>
              <span className='text-sm text-stone-600'>
                {selectedFile ? selectedFile.split('/').pop() : 'No file selected'}
              </span>
            </div>
            <p className='text-xs text-stone-500 mt-1'>
              Select a PDF with recipe text. AI will extract the recipe automatically.
            </p>
          </div>
          {fileError && (
            <div className='bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
              {fileError}
            </div>
          )}
          <div className='flex gap-2'>
            <button
              type='submit'
              disabled={!selectedFile || fileLoading}
              className='bg-primary-600 text-white px-4 py-2 rounded hover:bg-primary-700 disabled:opacity-50 disabled:cursor-wait'
            >
              {fileLoading ? 'Analyzing recipe...' : 'Import'}
            </button>
            <button
              type='button'
              onClick={onCancel}
              disabled={fileLoading}
              className='border border-stone-300 px-4 py-2 rounded hover:bg-stone-50 disabled:opacity-50'
            >
              Cancel
            </button>
          </div>
        </form>
      )}
    </div>
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
    <div className='border border-secondary-200 rounded-lg p-4 bg-secondary-50'>
      <h3 className='font-semibold text-lg mb-3'>Scale: {parsed.name}</h3>

      <div className='flex items-center gap-3 mb-4'>
        <span className='text-sm text-stone-600'>
          Current: {parsed.servings} serving{parsed.servings !== 1 ? 's' : ''}
        </span>
        <span className='text-stone-400'>{'\u2192'}</span>
        <input
          type='number'
          min={1}
          value={targetServings}
          onChange={(e) => setTargetServings(parseInt(e.target.value) || 1)}
          className='border border-stone-300 p-1 rounded w-20 text-sm'
        />
        <span className='text-sm text-stone-600'>servings</span>
        <button
          onClick={handlePreview}
          disabled={targetServings < 1 || targetServings === parsed.servings}
          className='bg-secondary-600 text-white px-3 py-1 rounded text-sm hover:bg-secondary-700 disabled:opacity-50 disabled:cursor-not-allowed'
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
                <span className='text-stone-500 w-12'>{ing.unit}</span>
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
              className='bg-primary-600 text-white px-4 py-2 rounded text-sm hover:bg-primary-700'
            >
              Save as New Recipe
            </button>
            <button
              onClick={() => onUpdateInPlace(editedIngredients, targetServings)}
              className='border border-stone-300 px-4 py-2 rounded text-sm hover:bg-stone-50'
            >
              Update This Recipe
            </button>
            <button
              onClick={onCancel}
              className='border border-stone-300 px-4 py-2 rounded text-sm text-stone-600 hover:bg-stone-50'
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
            className='border border-stone-300 px-4 py-2 rounded text-sm text-stone-600 hover:bg-stone-50'
          >
            Cancel
          </button>
        </div>
      )}
    </div>
  )
}

// --- Enhanced Instructions Renderer ---

function EnhancedInstructions({ text }: { text: string }) {
  // Render **bold** markdown as <strong> elements
  const renderLine = (line: string, lineIdx: number) => {
    const parts: React.ReactNode[] = []
    let remaining = line
    let key = 0

    while (remaining.length > 0) {
      const boldStart = remaining.indexOf('**')
      if (boldStart === -1) {
        parts.push(remaining)
        break
      }
      const boldEnd = remaining.indexOf('**', boldStart + 2)
      if (boldEnd === -1) {
        parts.push(remaining)
        break
      }
      // Text before bold
      if (boldStart > 0) {
        parts.push(remaining.slice(0, boldStart))
      }
      // Bold text
      parts.push(
        <strong key={`${lineIdx}-${key++}`} className='text-primary-700 font-semibold'>
          {remaining.slice(boldStart + 2, boldEnd)}
        </strong>,
      )
      remaining = remaining.slice(boldEnd + 2)
    }

    return parts
  }

  return (
    <>
      {text.split('\n').map((line, i) => (
        <span key={i}>
          {renderLine(line, i)}
          {i < text.split('\n').length - 1 && '\n'}
        </span>
      ))}
    </>
  )
}

// --- Recipe Detail View ---

function RecipeDetail({
  parsed,
  parentName,
  onEdit,
  onScale,
  onAdapt,
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
  onAdapt: () => void
  onDelete: () => void
  onToggleFavorite: () => void
  onRatingChange: (rating: number) => void
  onClose: () => void
  confirmingDelete: boolean
  onConfirmDelete: () => void
  onCancelDelete: () => void
}) {
  const [enhancedMode, setEnhancedMode] = useState(false)
  const [enhancedText, setEnhancedText] = useState<string | null>(null)
  const enhanceMutation = useEnhanceInstructions()

  const handleToggleEnhanced = () => {
    if (enhancedMode) {
      setEnhancedMode(false)
      return
    }
    // Fetch enhanced instructions if not cached
    if (enhancedText) {
      setEnhancedMode(true)
      return
    }
    enhanceMutation.mutate(parsed.id, {
      onSuccess: (text) => {
        setEnhancedText(text)
        setEnhancedMode(true)
      },
    })
  }

  return (
    <div className='border border-stone-200 p-6 rounded-lg bg-white shadow-sm md:col-span-2'>
      {/* Header */}
      <div className='flex items-start justify-between mb-4'>
        <div>
          <div className='flex items-center gap-2'>
            {parsed.icon && <span className='text-2xl'>{parsed.icon}</span>}
            <h2 className='text-xl font-bold'>{parsed.name}</h2>
            <button
              onClick={onToggleFavorite}
              className={`text-xl ${
                parsed.is_favorite ? 'text-accent-500' : 'text-stone-300 hover:text-accent-400'
              }`}
            >
              {parsed.is_favorite ? '\u2605' : '\u2606'}
            </button>
          </div>
          <div className='flex items-center gap-2 mt-1'>
            <StarRating value={parsed.rating} onChange={onRatingChange} size='md' />
          </div>
          {parsed.description && <p className='text-stone-600 mt-1'>{parsed.description}</p>}
        </div>
        <div className='flex gap-2 items-center'>
          <button
            onClick={onEdit}
            className='text-primary-600 text-sm hover:underline'
          >
            Edit
          </button>
          <button
            onClick={onScale}
            className='text-secondary-600 text-sm hover:underline'
          >
            Scale
          </button>
          <button
            onClick={onAdapt}
            className='text-secondary-600 text-sm hover:underline'
          >
            Adapt
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
                  className='text-stone-500 hover:underline'
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
            className='ml-2 text-stone-400 hover:text-stone-600 text-lg'
            title='Back to list'
          >
            {'\u2715'}
          </button>
        </div>
      </div>

      {/* Parent recipe link */}
      {parentName && (
        <div className='text-sm text-stone-500 mb-2'>
          {parsed.source === 'ai_adapted' ? 'Adapted from' : 'Scaled from'}:{' '}
          <span
            className={`font-medium ${
              parsed.source === 'ai_adapted' ? 'text-secondary-600' : 'text-secondary-600'
            }`}
          >
            {parentName}
          </span>
        </div>
      )}

      {/* Meta row */}
      <div className='flex flex-wrap gap-4 text-sm text-stone-500 mb-4 pb-4 border-b border-stone-100'>
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
                    {ing.unit && <span className='text-stone-500'>{` ${ing.unit}`}</span>}
                    <span>{` ${ing.name}`}</span>
                    {ing.notes && <span className='text-stone-400 italic'>{` (${ing.notes})`}
                    </span>}
                  </li>
                ))}
              </ul>
            )
            : <p className='text-sm text-stone-400'>No ingredients listed</p>}
        </div>

        {/* Instructions */}
        <div className='md:col-span-2'>
          <div className='flex items-center gap-2 mb-2'>
            <h3 className='font-semibold'>Instructions</h3>
            {parsed.instructions && (
              <button
                onClick={handleToggleEnhanced}
                disabled={enhanceMutation.isPending}
                className={`text-xs px-2 py-0.5 rounded border ${
                  enhancedMode
                    ? 'bg-primary-100 border-primary-300 text-primary-700'
                    : 'bg-stone-50 border-stone-200 text-stone-600 hover:bg-stone-100'
                } ${enhanceMutation.isPending ? 'opacity-50 cursor-wait' : ''}`}
              >
                {enhanceMutation.isPending
                  ? 'Loading...'
                  : enhancedMode
                  ? 'Enhanced \u2713'
                  : 'Enhanced view'}
              </button>
            )}
          </div>
          {parsed.instructions
            ? (
              <div className='text-sm whitespace-pre-wrap leading-relaxed'>
                {enhancedMode && enhancedText
                  ? <EnhancedInstructions text={enhancedText} />
                  : parsed.instructions}
              </div>
            )
            : <p className='text-sm text-stone-400'>No instructions</p>}
          {enhanceMutation.error && (
            <p className='text-sm text-red-600 mt-1'>
              {String(enhanceMutation.error)}
            </p>
          )}
        </div>
      </div>

      {/* Nutrition */}
      {parsed.nutrition_per_serving && (
        <div className='mt-4 pt-4 border-t border-stone-100'>
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
        <div className='flex flex-wrap gap-1 mt-4 pt-4 border-t border-stone-100'>
          {parsed.tags.map((tag, i) => (
            <span
              key={i}
              className='bg-stone-100 text-stone-600 text-xs px-2 py-0.5 rounded'
            >
              {tag}
            </span>
          ))}
        </div>
      )}

      {/* Notes */}
      {parsed.notes && (
        <div className='mt-4 pt-4 border-t border-stone-100'>
          <h3 className='font-semibold mb-1'>Notes</h3>
          <p className='text-sm text-stone-600 italic'>{parsed.notes}</p>
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
  const importUrlMutation = useImportRecipeFromUrl()
  const importFileMutation = useImportRecipeFromFile()

  const [searchQuery, setSearchQuery] = useState('')
  const [viewMode, setViewMode] = useState<'list' | 'add' | 'import'>('list')
  const [viewingId, setViewingId] = useState<string | null>(null)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [scalingId, setScalingId] = useState<string | null>(null)
  const [adaptingId, setAdaptingId] = useState<string | null>(null)
  const [adaptDraft, setAdaptDraft] = useState<CreateRecipeDto | null>(null)
  const [scaleError, setScaleError] = useState<string | null>(null)
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<string | null>(null)

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (adaptingId) {
          setAdaptingId(null)
          setAdaptDraft(null)
        } else if (scalingId) {
          setScalingId(null)
        } else if (editingId) {
          setEditingId(null)
          setAdaptDraft(null)
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
  }, [adaptingId, scalingId, editingId, viewingId, viewMode])

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

  const handleImportFromUrl = (url: string) => {
    importUrlMutation.mutate({ url }, {
      onSuccess: () => setViewMode('list'),
    })
  }

  const handleImportFromFile = (filePath: string) => {
    importFileMutation.mutate({ file_path: filePath }, {
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

  const handleAdaptDraftSave = (formData: RecipeFormData) => {
    if (!adaptDraft) return
    const dto: CreateRecipeDto = {
      name: formData.name,
      source: adaptDraft.source,
      parent_recipe_id: adaptDraft.parent_recipe_id,
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
      onSuccess: (recipe) => {
        setEditingId(null)
        setAdaptDraft(null)
        setViewingId(recipe.id)
      },
    })
  }

  if (isLoading) {
    return <div className='p-6 text-stone-500 animate-pulse'>Loading recipes...</div>
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
        <h1 className='text-2xl font-bold text-stone-900'>Recipes</h1>
        {viewMode === 'list' && (
          <div className='flex gap-2'>
            <button
              onClick={() => setViewMode('add')}
              className='bg-primary-600 text-white px-4 py-2 rounded hover:bg-primary-700'
            >
              + Add Recipe
            </button>
            <button
              onClick={() => setViewMode('import')}
              className='border border-stone-300 px-4 py-2 rounded hover:bg-stone-50'
            >
              Import
            </button>
          </div>
        )}
      </div>

      {viewMode === 'add' && (
        <div className='mb-6 border border-stone-200 p-4 rounded-lg bg-white shadow-sm'>
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
        <div className='mb-6 border border-stone-200 p-4 rounded-lg bg-white shadow-sm'>
          <h3 className='font-semibold text-lg mb-3'>Import Recipe</h3>
          <ImportRecipeForm
            onSubmitMarkdown={handleImport}
            onSubmitUrl={handleImportFromUrl}
            onSubmitFile={handleImportFromFile}
            onCancel={() => setViewMode('list')}
            markdownError={importMutation.error ? String(importMutation.error) : undefined}
            urlError={importUrlMutation.error ? String(importUrlMutation.error) : undefined}
            fileError={importFileMutation.error ? String(importFileMutation.error) : undefined}
            urlLoading={importUrlMutation.isPending}
            fileLoading={importFileMutation.isPending}
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
            className='border border-stone-300 p-2 rounded w-full md:w-64'
          />
        </div>
      )}

      {filteredRecipes?.length === 0 && viewMode === 'list' && (
        <p className='text-stone-500'>
          {searchQuery
            ? 'No recipes match your search.'
            : 'No recipes yet. Add one to get started!'}
        </p>
      )}

      <div className='grid grid-cols-1 md:grid-cols-2 gap-4'>
        {filteredRecipes?.map((recipe) => {
          const parsed = parseRecipe(recipe)

          if (editingId === recipe.id) {
            const isAdaptEdit = !!adaptDraft
            const formInitial = isAdaptEdit
              ? {
                name: adaptDraft.name,
                description: adaptDraft.description || '',
                prep_time: adaptDraft.prep_time,
                cook_time: adaptDraft.cook_time,
                total_time: adaptDraft.total_time,
                servings: adaptDraft.servings,
                instructions: adaptDraft.instructions,
                ingredients: adaptDraft.ingredients,
                tags: adaptDraft.tags,
                notes: adaptDraft.notes || '',
                icon: adaptDraft.icon || '',
              }
              : {
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
              }
            return (
              <div
                key={recipe.id}
                className={`p-4 rounded-lg md:col-span-2 border ${
                  isAdaptEdit
                    ? 'border-secondary-200 bg-secondary-50'
                    : 'border-primary-200 bg-primary-50'
                }`}
              >
                <h3 className='font-semibold text-lg mb-3'>
                  {isAdaptEdit ? 'Edit Adapted Recipe' : `Edit ${recipe.name}`}
                </h3>
                <RecipeForm
                  initialData={formInitial}
                  onSubmit={isAdaptEdit
                    ? handleAdaptDraftSave
                    : (data) => handleUpdate(recipe.id, data)}
                  onCancel={() => {
                    setEditingId(null)
                    setAdaptDraft(null)
                  }}
                  submitLabel={isAdaptEdit ? 'Save Adapted Recipe' : 'Save Changes'}
                />
                {(isAdaptEdit ? createMutation.error : updateMutation.error) && (
                  <div className='mt-2 bg-red-50 border border-red-200 rounded p-3 text-red-700 text-sm'>
                    {String(isAdaptEdit ? createMutation.error : updateMutation.error)}
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

          if (adaptingId === recipe.id) {
            return (
              <div key={recipe.id} className='md:col-span-2'>
                <AdaptRecipePanel
                  parsed={parsed}
                  onComplete={(newId) => {
                    setAdaptingId(null)
                    setViewingId(newId)
                  }}
                  onEdit={(draft) => {
                    setAdaptDraft(draft)
                    setAdaptingId(null)
                    setEditingId(recipe.id)
                  }}
                  onCancel={() => setAdaptingId(null)}
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
                onAdapt={() => {
                  setViewingId(null)
                  setAdaptingId(recipe.id)
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
              className='border border-stone-200 p-4 rounded-lg bg-white shadow-sm'
            >
              <div className='flex items-start justify-between'>
                <div className='flex items-center gap-2'>
                  {recipe.icon && <span className='text-xl'>{recipe.icon}</span>}
                  <button
                    onClick={() => setViewingId(recipe.id)}
                    className='font-semibold text-lg hover:text-primary-600 text-left'
                  >
                    {recipe.name}
                  </button>
                  <button
                    onClick={() => toggleFavoriteMutation.mutate(recipe.id)}
                    className={`text-lg ${
                      recipe.is_favorite
                        ? 'text-accent-500'
                        : 'text-stone-300 hover:text-accent-400'
                    }`}
                    title={recipe.is_favorite ? 'Unfavorite' : 'Favorite'}
                  >
                    {recipe.is_favorite ? '\u2605' : '\u2606'}
                  </button>
                  <StarRating value={recipe.rating} size='sm' />
                </div>
              </div>

              {recipe.description && (
                <p className='text-sm text-stone-600 mt-1'>{recipe.description}</p>
              )}

              <div className='flex gap-4 mt-2 text-sm text-stone-500'>
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
                      className='bg-stone-100 text-stone-600 text-xs px-2 py-0.5 rounded'
                    >
                      {tag}
                    </span>
                  ))}
                </div>
              )}

              {recipe.times_made > 0 && (
                <p className='text-xs text-stone-400 mt-2'>
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
