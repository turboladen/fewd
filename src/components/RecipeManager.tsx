import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  useCreateRecipe,
  useEnhanceInstructions,
  useImportRecipe,
  useImportRecipeFromFile,
  useImportRecipeFromUrl,
  usePreviewScaleRecipe,
  useRecipes,
  useToggleFavorite,
} from '../hooks/useRecipes'
import type {
  CreateRecipeDto,
  Ingredient,
  ParsedRecipe,
  PortionSize,
  ScaleResult,
  TimeValue,
} from '../types/recipe'
import { formatAmount, formatServings, formatTime, parseRecipe } from '../types/recipe'
import { EmptyState } from './EmptyState'
import {
  IconArrowRight,
  IconCheck,
  IconClose,
  IconEdit,
  IconPlus,
  IconRefresh,
  IconStar,
  IconStarFilled,
  IconTrash,
} from './Icon'
import { IngredientInput } from './IngredientInput'
import { NumberInput } from './NumberInput'
import { StarRating } from './StarRating'
import { TagInput } from './TagInput'
import { useToast } from './Toast'

// --- Sub-components ---

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
          className='input-sm w-20'
        />
        <select
          value={value?.unit ?? 'minutes'}
          onChange={(e) => {
            if (value) {
              onChange({ ...value, unit: e.target.value as TimeValue['unit'] })
            }
          }}
          className='input-sm'
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

export interface RecipeFormData {
  name: string
  description: string
  prep_time?: TimeValue
  cook_time?: TimeValue
  total_time?: TimeValue
  servings: number
  portion_size?: PortionSize
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

export function RecipeForm({
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
            className='input w-full'
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
            className='input w-24'
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
          className='input w-full'
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
          <NumberInput
            value={form.servings}
            onChange={(servings) => setForm({ ...form, servings })}
            min={1}
            required
            className='input-sm w-20'
          />
        </div>
      </div>

      <div>
        <label className='block text-sm font-medium text-stone-700 mb-1'>
          Serving size <span className='text-stone-400 font-normal'>(optional)</span>
        </label>
        <div className='flex flex-wrap gap-2 items-center'>
          <input
            type='number'
            min={0}
            step='any'
            value={form.portion_size?.value ?? ''}
            onChange={(e) => {
              const v = e.target.value
              if (v === '' && !form.portion_size?.unit) {
                setForm({ ...form, portion_size: undefined })
              } else {
                setForm({
                  ...form,
                  portion_size: {
                    value: parseFloat(v) || 0,
                    unit: form.portion_size?.unit ?? '',
                  },
                })
              }
            }}
            placeholder='2'
            className='input-sm w-20'
          />
          <input
            type='text'
            value={form.portion_size?.unit ?? ''}
            onChange={(e) => {
              const unit = e.target.value
              if (!unit && !form.portion_size?.value) {
                setForm({ ...form, portion_size: undefined })
              } else {
                setForm({
                  ...form,
                  portion_size: {
                    value: form.portion_size?.value ?? 0,
                    unit,
                  },
                })
              }
            }}
            placeholder='cookies, slices, pieces...'
            className='input-sm w-full sm:w-48'
          />
          <span className='text-xs text-stone-400'>per serving</span>
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
          className='input w-full'
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
          className='input w-full'
          rows={2}
        />
      </div>

      {validationError && (
        <div className='panel-error text-red-700 text-sm'>
          {validationError}
        </div>
      )}

      <div className='flex gap-2'>
        <button
          type='submit'
          className='btn-md btn-primary'
        >
          {submitLabel}
        </button>
        <button
          type='button'
          onClick={onCancel}
          className='btn-md btn-outline'
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
  urlLoadingMessage,
  fileLoading,
}: {
  onSubmitMarkdown: (markdown: string) => void
  onSubmitUrl: (url: string) => void
  onSubmitFile: (file: File) => void
  onCancel: () => void
  markdownError?: string
  urlError?: string
  fileError?: string
  urlLoading?: boolean
  urlLoadingMessage?: string
  fileLoading?: boolean
}) {
  const [importMode, setImportMode] = useState<'markdown' | 'url' | 'pdf'>('url')
  const [markdown, setMarkdown] = useState('')
  const [url, setUrl] = useState('')
  const [selectedFile, setSelectedFile] = useState<File | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const handleMarkdownSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmitMarkdown(markdown)
  }

  const handleUrlSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmitUrl(url)
  }

  const handleChooseFile = () => {
    fileInputRef.current?.click()
  }

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (file) {
      setSelectedFile(file)
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
              className='input w-full font-mono'
              rows={12}
              placeholder={`# Recipe Name\nDescription here\nPrep time: 15 min\nServings: 4\n\n## Ingredients\n- 2 cups flour\n- 1 tsp salt\n\n## Instructions\n1. Mix ingredients...\n\n## Tags\ndinner, quick`}
            />
          </div>
          {markdownError && (
            <div className='panel-error text-red-700 text-sm'>
              {markdownError}
            </div>
          )}
          <div className='flex gap-2'>
            <button
              type='submit'
              className='btn-md btn-primary'
            >
              Import
            </button>
            <button
              type='button'
              onClick={onCancel}
              className='btn-md btn-outline'
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
              className='input w-full'
            />
            <p className='text-xs text-stone-500 mt-1'>
              Paste a link to a recipe page. AI will extract the recipe automatically.
            </p>
          </div>
          {urlError && (
            <div className='panel-error text-red-700 text-sm'>
              {urlError}
            </div>
          )}
          <div className='flex gap-2'>
            <button
              type='submit'
              disabled={urlLoading}
              className='btn-md btn-primary disabled:cursor-wait'
            >
              {urlLoading ? (urlLoadingMessage || 'Analyzing recipe...') : 'Import'}
            </button>
            <button
              type='button'
              onClick={onCancel}
              disabled={urlLoading}
              className='btn-md btn-outline'
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
              <input
                ref={fileInputRef}
                type='file'
                accept='.pdf'
                onChange={handleFileChange}
                className='hidden'
              />
              <button
                type='button'
                onClick={handleChooseFile}
                className='btn-sm btn-outline'
              >
                Choose File
              </button>
              <span className='text-sm text-stone-600'>
                {selectedFile ? selectedFile.name : 'No file selected'}
              </span>
            </div>
            <p className='text-xs text-stone-500 mt-1'>
              Select a PDF with recipe text. AI will extract the recipe automatically.
            </p>
          </div>
          {fileError && (
            <div className='panel-error text-red-700 text-sm'>
              {fileError}
            </div>
          )}
          <div className='flex gap-2'>
            <button
              type='submit'
              disabled={!selectedFile || fileLoading}
              className='btn-md btn-primary disabled:cursor-wait'
            >
              {fileLoading ? 'Analyzing recipe...' : 'Import'}
            </button>
            <button
              type='button'
              onClick={onCancel}
              disabled={fileLoading}
              className='btn-md btn-outline'
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

export function ScaleRecipePanel({
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
    <div className='panel-secondary animate-slide-up'>
      <h3 className='font-semibold text-lg mb-3'>Scale: {parsed.name}</h3>

      <div className='flex items-center gap-3 mb-4'>
        <span className='text-sm text-stone-600'>
          Current: {parsed.servings} serving{parsed.servings !== 1 ? 's' : ''}
        </span>
        <span className='text-stone-400'>
          <IconArrowRight className='w-4 h-4' />
        </span>
        <NumberInput
          value={targetServings}
          onChange={setTargetServings}
          min={1}
          className='input-sm w-20'
        />
        <span className='text-sm text-stone-600'>servings</span>
        <button
          onClick={handlePreview}
          disabled={targetServings < 1 || targetServings === parsed.servings}
          className='btn-sm btn-secondary'
        >
          Preview
        </button>
      </div>

      {previewMutation.error && (
        <div className='mb-3 panel-error text-red-700 text-sm'>
          {String(previewMutation.error)}
        </div>
      )}

      {preview && editedIngredients && (
        <>
          {preview.flagged.length > 0 && (
            <div className='mb-3 panel-warning text-amber-800 text-sm'>
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
                      className='input-sm w-16 border-amber-300'
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
            <div className='mb-3 panel-error text-red-700 text-sm'>
              {error}
            </div>
          )}

          <div className='flex gap-2'>
            <button
              onClick={() => onSaveAsNew(editedIngredients, targetServings)}
              className='btn-sm btn-primary'
            >
              Save as New Recipe
            </button>
            <button
              onClick={() => onUpdateInPlace(editedIngredients, targetServings)}
              className='btn-sm btn-outline'
            >
              Update This Recipe
            </button>
            <button
              onClick={onCancel}
              className='btn-sm btn-outline'
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
            className='btn-sm btn-outline'
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

export function RecipeDetail({
  parsed,
  parentName,
  onEdit,
  onScale,
  onAdapt,
  onCook,
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
  onCook: () => void
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
    <div className='card p-6 md:col-span-2 animate-fade-in'>
      {/* Header */}
      <div className='flex flex-col sm:flex-row items-start sm:justify-between gap-2 mb-4'>
        <div>
          <div className='flex items-center gap-2'>
            {parsed.icon && <span className='text-2xl'>{parsed.icon}</span>}
            <h2 className='text-xl font-semibold'>{parsed.name}</h2>
            <button
              onClick={onToggleFavorite}
              className={`text-xl ${
                parsed.is_favorite ? 'text-accent-500' : 'text-stone-300 hover:text-accent-400'
              }`}
              aria-label={parsed.is_favorite ? 'Remove from favorites' : 'Add to favorites'}
            >
              {parsed.is_favorite
                ? <IconStarFilled className='w-5 h-5' />
                : <IconStar className='w-5 h-5' />}
            </button>
          </div>
          <div className='flex items-center gap-2 mt-1'>
            <StarRating value={parsed.rating} onChange={onRatingChange} size='md' />
          </div>
          {parsed.description && <p className='text-stone-600 mt-1'>{parsed.description}</p>}
        </div>
        <div className='flex flex-wrap gap-1.5 items-center'>
          <button onClick={onCook} className='btn-sm btn-primary'>
            Cook this
          </button>
          <span className='text-stone-200 mx-0.5' aria-hidden='true'>|</span>
          <button onClick={onEdit} className='btn-xs btn-outline'>
            <IconEdit className='w-3.5 h-3.5' />
            Edit
          </button>
          <button onClick={onScale} className='btn-xs btn-outline'>
            Scale
          </button>
          <button onClick={onAdapt} className='btn-xs btn-outline'>
            <IconRefresh className='w-3.5 h-3.5' />
            Adapt
          </button>
          <span className='text-stone-200 mx-0.5' aria-hidden='true'>|</span>
          {confirmingDelete
            ? (
              <span className='flex gap-1 items-center'>
                <span className='text-red-600 text-xs'>Delete?</span>
                <button onClick={onConfirmDelete} className='btn-xs btn-danger-solid'>
                  Yes
                </button>
                <button onClick={onCancelDelete} className='btn-xs btn-outline'>
                  No
                </button>
              </span>
            )
            : (
              <button
                onClick={onDelete}
                className='btn-xs border border-red-200 text-red-600 hover:bg-red-50 hover:border-red-300'
              >
                <IconTrash className='w-3.5 h-3.5' />
                Delete
              </button>
            )}
          <button
            onClick={onClose}
            className='ml-auto p-1 text-stone-400 hover:text-stone-600 hover:bg-stone-100 rounded'
            title='Back to list'
          >
            <IconClose className='w-5 h-5' />
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

      {/* Source URL */}
      {parsed.source_url && (
        <div className='text-sm text-stone-500 mb-2'>
          Source:{' '}
          <a
            href={parsed.source_url}
            target='_blank'
            rel='noopener noreferrer'
            className='text-primary-600 hover:text-primary-700 underline'
          >
            {(() => {
              try {
                return new URL(parsed.source_url).hostname.replace(/^www\./, '')
              } catch {
                return parsed.source_url
              }
            })()}
          </a>
        </div>
      )}

      {/* Meta row */}
      <div className='flex flex-wrap gap-4 text-sm text-stone-500 mb-4 pb-4 border-b border-stone-100'>
        <span>Servings: {formatServings(parsed.servings, parsed.portion_size)}</span>
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
                  ? (
                    <>
                      Enhanced <IconCheck className='w-3 h-3 inline' />
                    </>
                  )
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
              className='tag'
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
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const { toast } = useToast()
  const { data: recipes, isLoading, error } = useRecipes()
  const createMutation = useCreateRecipe()
  const toggleFavoriteMutation = useToggleFavorite()
  const importMutation = useImportRecipe()
  const importUrlMutation = useImportRecipeFromUrl()
  const importFileMutation = useImportRecipeFromFile()

  const [searchQuery, setSearchQuery] = useState('')
  const [viewMode, setViewMode] = useState<'list' | 'add' | 'import'>('list')

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && viewMode !== 'list') {
        setViewMode('list')
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [viewMode])

  const filteredRecipes = recipes?.filter((r) =>
    r.name.toLowerCase().includes(searchQuery.toLowerCase())
  )

  const handleCreate = (formData: RecipeFormData) => {
    const dto: CreateRecipeDto = {
      name: formData.name,
      source: 'manual',
      servings: formData.servings,
      portion_size: formData.portion_size,
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
        toast('Recipe created')
        setViewMode('list')
        navigate(`/recipes/${recipe.slug}`)
      },
    })
  }

  const handleImport = (markdown: string) => {
    importMutation.mutate({ markdown }, {
      onSuccess: (recipe) => {
        toast('Recipe imported')
        setViewMode('list')
        navigate(`/recipes/${recipe.slug}`)
      },
    })
  }

  const handleImportFromUrl = (url: string) => {
    importUrlMutation.mutate({ url }, {
      onSuccess: (recipe) => {
        queryClient.invalidateQueries({ queryKey: ['recipes'] })
        toast('Recipe imported from URL')
        setViewMode('list')
        navigate(`/recipes/${recipe.slug}`)
      },
    })
  }

  const handleImportFromFile = (file: File) => {
    importFileMutation.mutate(file, {
      onSuccess: (recipe) => {
        toast('Recipe imported from PDF')
        setViewMode('list')
        navigate(`/recipes/${recipe.slug}`)
      },
    })
  }

  if (isLoading) {
    return <div className='p-6 text-stone-500 animate-pulse'>Loading recipes...</div>
  }

  if (error) {
    return (
      <div className='p-6'>
        <div className='panel-error text-red-700 text-sm'>
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
              className='btn-md btn-primary'
            >
              <IconPlus className='w-4 h-4' /> Add Recipe
            </button>
            <button
              onClick={() => setViewMode('import')}
              className='btn-md btn-outline'
            >
              Import
            </button>
          </div>
        )}
      </div>

      {viewMode === 'add' && (
        <div className='mb-6 card p-4 animate-slide-up'>
          <h3 className='font-semibold text-lg mb-3'>Add Recipe</h3>
          <RecipeForm
            initialData={emptyForm}
            onSubmit={handleCreate}
            onCancel={() => setViewMode('list')}
            submitLabel='Add Recipe'
          />
          {createMutation.error && (
            <div className='mt-2 panel-error text-red-700 text-sm'>
              {String(createMutation.error)}
            </div>
          )}
        </div>
      )}

      {viewMode === 'import' && (
        <div className='mb-6 card p-4 animate-slide-up'>
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
            urlLoadingMessage={importUrlMutation.progress?.message}
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
            className='input w-full md:w-64'
          />
        </div>
      )}

      {filteredRecipes?.length === 0 && viewMode === 'list' && (
        searchQuery
          ? <p className='text-stone-500'>No recipes match your search.</p>
          : (
            <EmptyState
              emoji='📖'
              title='Your recipe book is empty'
              description='Add a recipe manually or import one to get started.'
              action={{ label: 'Add Recipe', onClick: () => setViewMode('add') }}
            />
          )
      )}

      <div className='grid grid-cols-1 md:grid-cols-2 gap-4'>
        {filteredRecipes?.map((recipe) => {
          const parsed = parseRecipe(recipe)

          return (
            <div
              key={recipe.id}
              className='card-hover p-4 animate-slide-up'
            >
              <div className='flex items-start justify-between'>
                <div className='flex items-center gap-2'>
                  {recipe.icon && <span className='text-xl'>{recipe.icon}</span>}
                  <button
                    onClick={() => navigate(`/recipes/${recipe.slug}`)}
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
                    {recipe.is_favorite
                      ? <IconStarFilled className='w-5 h-5' />
                      : <IconStar className='w-5 h-5' />}
                  </button>
                  <StarRating value={recipe.rating} size='sm' />
                </div>
              </div>

              {recipe.description && (
                <p className='text-sm text-stone-600 mt-1'>{recipe.description}</p>
              )}

              <div className='flex gap-4 mt-2 text-sm text-stone-500'>
                <span>Servings: {formatServings(recipe.servings, parsed.portion_size)}</span>
                {parsed.prep_time && <span>Prep: {formatTime(parsed.prep_time)}</span>}
                {parsed.cook_time && <span>Cook: {formatTime(parsed.cook_time)}</span>}
                {parsed.total_time && <span>Total: {formatTime(parsed.total_time)}</span>}
              </div>

              {parsed.tags.length > 0 && (
                <div className='flex flex-wrap gap-1 mt-2'>
                  {parsed.tags.map((tag, i) => (
                    <span
                      key={i}
                      className='tag'
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
