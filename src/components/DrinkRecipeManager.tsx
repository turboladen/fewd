import { useEffect, useRef, useState } from 'react'
import {
  useCreateDrinkRecipe,
  useDeleteDrinkRecipe,
  useDrinkRecipes,
  useImportDrinkRecipeFromUrl,
  useToggleDrinkFavorite,
  useUpdateDrinkRecipe,
} from '../hooks/useDrinkRecipes'
import type {
  CreateDrinkRecipeDto,
  DrinkRecipeFormData,
  UpdateDrinkRecipeDto,
} from '../types/drinkRecipe'
import { emptyDrinkRecipeForm, parseDrinkRecipe } from '../types/drinkRecipe'
import { formatAmount } from '../types/recipe'
import { DrinkRecipeForm } from './DrinkRecipeForm'
import { EmptyState } from './EmptyState'
import {
  IconChevronDown,
  IconChevronRight,
  IconEdit,
  IconPlus,
  IconSearch,
  IconStar,
  IconStarFilled,
  IconTrash,
} from './Icon'
import { StarRating } from './StarRating'
import { useToast } from './Toast'

export function DrinkRecipeManager({ onSwitchToSuggest }: { onSwitchToSuggest: () => void }) {
  const { data: drinkRecipes, isLoading, error } = useDrinkRecipes()
  const createMutation = useCreateDrinkRecipe()
  const updateMutation = useUpdateDrinkRecipe()
  const deleteMutation = useDeleteDrinkRecipe()
  const toggleFavMutation = useToggleDrinkFavorite()
  const importUrlMutation = useImportDrinkRecipeFromUrl()
  const { toast } = useToast()

  const [viewMode, setViewMode] = useState<'list' | 'add' | 'import'>('list')
  const [importUrl, setImportUrl] = useState('')
  const [editingId, setEditingId] = useState<string | null>(null)
  const [searchQuery, setSearchQuery] = useState('')
  const [expandedId, setExpandedId] = useState<string | null>(null)
  const [highlightId, setHighlightId] = useState<string | null>(null)
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<string | null>(null)
  const highlightRef = useRef<HTMLDivElement>(null)

  // Scroll to and highlight a newly imported recipe
  useEffect(() => {
    if (highlightId && highlightRef.current) {
      highlightRef.current.scrollIntoView({ behavior: 'smooth', block: 'center' })
      const timer = setTimeout(() => setHighlightId(null), 2000)
      return () => clearTimeout(timer)
    }
  }, [highlightId, drinkRecipes])

  // Escape key to close add/edit mode
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (editingId) {
          setEditingId(null)
        } else if (viewMode === 'add' || viewMode === 'import') {
          setViewMode('list')
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [editingId, viewMode])

  const handleCreate = (formData: DrinkRecipeFormData) => {
    const dto: CreateDrinkRecipeDto = {
      name: formData.name,
      source: 'manual',
      servings: formData.servings,
      instructions: formData.instructions,
      ingredients: formData.ingredients,
      technique: formData.technique || undefined,
      glassware: formData.glassware || undefined,
      garnish: formData.garnish || undefined,
      tags: formData.tags,
      description: formData.description || undefined,
      notes: formData.notes || undefined,
      icon: formData.icon || undefined,
      is_non_alcoholic: formData.is_non_alcoholic,
    }
    createMutation.mutate(dto, {
      onSuccess: () => {
        setViewMode('list')
        toast('Drink recipe added')
      },
    })
  }

  const handleUpdate = (formData: DrinkRecipeFormData) => {
    if (!editingId) return
    const dto: UpdateDrinkRecipeDto = {
      name: formData.name,
      servings: formData.servings,
      instructions: formData.instructions,
      ingredients: formData.ingredients,
      technique: formData.technique || undefined,
      glassware: formData.glassware || undefined,
      garnish: formData.garnish || undefined,
      tags: formData.tags,
      description: formData.description || undefined,
      notes: formData.notes || undefined,
      icon: formData.icon || undefined,
      is_non_alcoholic: formData.is_non_alcoholic,
    }
    updateMutation.mutate({ id: editingId, data: dto }, {
      onSuccess: () => {
        setEditingId(null)
        toast('Drink recipe updated')
      },
    })
  }

  const handleDelete = (id: string) => {
    deleteMutation.mutate(id, {
      onSuccess: () => {
        setConfirmingDeleteId(null)
        toast('Drink recipe deleted')
      },
    })
  }

  const handleToggleFavorite = (id: string) => {
    toggleFavMutation.mutate(id)
  }

  const handleImportFromUrl = (e: React.FormEvent) => {
    e.preventDefault()
    importUrlMutation.mutate({ url: importUrl }, {
      onSuccess: (recipe) => {
        toast(`Imported "${recipe.name}"`)
        setImportUrl('')
        setExpandedId(recipe.id)
        setHighlightId(recipe.id)
        setViewMode('list')
      },
    })
  }

  const handleRate = (id: string, rating: number) => {
    updateMutation.mutate({ id, data: { rating } })
  }

  const startEdit = (recipeId: string) => {
    setEditingId(recipeId)
    setExpandedId(null)
  }

  if (isLoading) {
    return <div className='p-6 text-stone-500 animate-pulse'>Loading drink recipes...</div>
  }

  if (error) {
    return (
      <div className='p-6'>
        <div className='panel-error text-red-700 text-sm'>
          Failed to load drink recipes: {String(error)}
        </div>
      </div>
    )
  }

  // --- Add mode ---
  if (viewMode === 'add') {
    return (
      <div className='p-6'>
        <h2 className='text-lg font-semibold text-stone-900 mb-4'>Add Drink Recipe</h2>
        <div className='card p-4 animate-slide-up'>
          <DrinkRecipeForm
            initialData={emptyDrinkRecipeForm}
            onSubmit={handleCreate}
            onCancel={() => setViewMode('list')}
            submitLabel='Add Recipe'
          />
        </div>
      </div>
    )
  }

  // --- Import mode ---
  if (viewMode === 'import') {
    return (
      <div className='p-6'>
        <h2 className='text-lg font-semibold text-stone-900 mb-4'>Import Drink Recipe</h2>
        <div className='card p-4 animate-slide-up'>
          <form onSubmit={handleImportFromUrl} className='space-y-3'>
            <div>
              <label className='block text-sm font-medium text-stone-700 mb-1'>
                Recipe URL
              </label>
              <input
                type='url'
                required
                value={importUrl}
                onChange={(e) => setImportUrl(e.target.value)}
                placeholder='https://example.com/cocktail/...'
                className='input w-full'
                autoFocus
              />
              <p className='text-xs text-stone-500 mt-1'>
                Paste a link to a cocktail recipe page. AI will extract the recipe automatically.
              </p>
            </div>
            {importUrlMutation.error && (
              <div className='panel-error text-red-700 text-sm'>
                {importUrlMutation.error instanceof Error
                  ? importUrlMutation.error.message
                  : String(importUrlMutation.error)}
              </div>
            )}
            <div className='flex gap-2'>
              <button
                type='submit'
                disabled={importUrlMutation.isPending}
                className='btn-md btn-primary disabled:cursor-wait'
              >
                {importUrlMutation.isPending ? 'Analyzing recipe...' : 'Import'}
              </button>
              <button
                type='button'
                onClick={() => setViewMode('list')}
                disabled={importUrlMutation.isPending}
                className='btn-md btn-outline'
              >
                Cancel
              </button>
            </div>
          </form>
        </div>
      </div>
    )
  }

  // --- Edit mode ---
  if (editingId) {
    const recipe = drinkRecipes?.find((r) => r.id === editingId)
    if (!recipe) {
      setEditingId(null)
      return null
    }
    const parsed = parseDrinkRecipe(recipe)
    const initialData: DrinkRecipeFormData = {
      name: recipe.name,
      description: recipe.description || '',
      icon: recipe.icon || '',
      servings: recipe.servings,
      instructions: recipe.instructions,
      ingredients: parsed.ingredients,
      technique: recipe.technique || '',
      glassware: recipe.glassware || '',
      garnish: recipe.garnish || '',
      tags: parsed.tags,
      notes: recipe.notes || '',
      is_non_alcoholic: recipe.is_non_alcoholic,
    }

    return (
      <div className='p-6'>
        <h2 className='text-lg font-semibold text-stone-900 mb-4'>Edit: {recipe.name}</h2>
        <div className='card p-4 animate-slide-up'>
          <DrinkRecipeForm
            initialData={initialData}
            onSubmit={handleUpdate}
            onCancel={() => setEditingId(null)}
            submitLabel='Save Changes'
          />
        </div>
      </div>
    )
  }

  // --- List mode ---

  const filtered = (drinkRecipes ?? []).filter((r) =>
    r.name.toLowerCase().includes(searchQuery.toLowerCase())
  )

  if (!drinkRecipes || drinkRecipes.length === 0) {
    return (
      <div className='p-6'>
        <div className='flex items-center justify-between mb-6'>
          <h2 className='text-lg font-semibold text-stone-900'>Drink Recipes</h2>
          <div className='flex items-center gap-2'>
            <button onClick={() => setViewMode('import')} className='btn-sm btn-outline'>
              Import
            </button>
            <button onClick={() => setViewMode('add')} className='btn-sm btn-primary'>
              <IconPlus className='w-4 h-4' />
              Add Recipe
            </button>
          </div>
        </div>
        <EmptyState
          emoji='🍹'
          title='No drink recipes yet'
          description='Add your own recipes or use the Suggester to discover new cocktails.'
          action={{ label: 'Go to Suggester', onClick: onSwitchToSuggest }}
        />
      </div>
    )
  }

  return (
    <div className='p-6'>
      {/* Header */}
      <div className='flex items-center justify-between mb-4'>
        <h2 className='text-lg font-semibold text-stone-900'>Drink Recipes</h2>
        <div className='flex items-center gap-2'>
          <button onClick={() => setViewMode('import')} className='btn-sm btn-outline'>
            Import
          </button>
          <button onClick={() => setViewMode('add')} className='btn-sm btn-primary'>
            <IconPlus className='w-4 h-4' />
            Add Recipe
          </button>
        </div>
      </div>

      {/* Search */}
      {drinkRecipes.length > 3 && (
        <div className='relative mb-4'>
          <IconSearch className='w-4 h-4 text-stone-400 absolute left-3 top-1/2 -translate-y-1/2' />
          <input
            type='text'
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder='Search drinks...'
            className='input w-full pl-9'
          />
        </div>
      )}

      {/* Recipe grid */}
      <div className='grid grid-cols-1 md:grid-cols-2 gap-4'>
        {filtered.map((recipe) => {
          const parsed = parseDrinkRecipe(recipe)
          const isExpanded = expandedId === recipe.id

          const isHighlighted = highlightId === recipe.id

          return (
            <div
              key={recipe.id}
              ref={isHighlighted ? highlightRef : undefined}
              className={`card p-4 animate-slide-up transition-shadow duration-1000 ${
                isHighlighted ? 'ring-2 ring-primary-400 shadow-lg' : ''
              }`}
            >
              <div className='flex items-start justify-between'>
                <button
                  onClick={() => setExpandedId(isExpanded ? null : recipe.id)}
                  className='flex items-center gap-2 text-left flex-1 min-w-0'
                >
                  <span className='text-xl flex-shrink-0'>{recipe.icon || '🍸'}</span>
                  <div className='min-w-0'>
                    <h3 className='font-semibold text-stone-900 truncate'>{recipe.name}</h3>
                    {recipe.description && (
                      <p className='text-sm text-stone-500 line-clamp-1'>{recipe.description}</p>
                    )}
                  </div>
                  {isExpanded
                    ? <IconChevronDown className='w-4 h-4 text-stone-400 flex-shrink-0' />
                    : <IconChevronRight className='w-4 h-4 text-stone-400 flex-shrink-0' />}
                </button>

                <div className='flex items-center gap-2 flex-shrink-0 ml-2'>
                  <button
                    onClick={() => handleToggleFavorite(recipe.id)}
                    className='text-amber-500 hover:text-amber-600'
                    aria-label={recipe.is_favorite ? 'Unfavorite' : 'Favorite'}
                  >
                    {recipe.is_favorite
                      ? <IconStarFilled className='w-5 h-5' />
                      : <IconStar className='w-5 h-5' />}
                  </button>
                </div>
              </div>

              {/* Meta tags */}
              <div className='flex flex-wrap gap-2 mt-2'>
                {recipe.technique && <span className='tag text-xs'>{recipe.technique}</span>}
                {recipe.glassware && <span className='tag text-xs'>{recipe.glassware}</span>}
                {recipe.is_non_alcoholic && (
                  <span className='tag text-xs bg-green-100 text-green-700'>non-alcoholic</span>
                )}
                {parsed.tags.map((tag) => <span key={tag} className='tag text-xs'>{tag}</span>)}
              </div>

              {/* Rating */}
              <div className='mt-2'>
                <StarRating
                  value={recipe.rating ?? 0}
                  onChange={(rating) => handleRate(recipe.id, rating)}
                  size='sm'
                />
              </div>

              {/* Expanded details */}
              {isExpanded && (
                <div className='mt-4 pt-4 border-t border-stone-200 space-y-3 animate-fade-in'>
                  {/* Ingredients */}
                  <div>
                    <h4 className='text-sm font-semibold text-stone-700 mb-1'>Ingredients</h4>
                    <ul className='text-sm text-stone-600 space-y-0.5'>
                      {parsed.ingredients.map((ing, i) => (
                        <li key={i}>
                          {formatAmount(ing.amount)} {ing.unit} {ing.name}
                          {ing.notes && <span className='text-stone-400'>({ing.notes})</span>}
                        </li>
                      ))}
                    </ul>
                  </div>

                  {/* Instructions */}
                  <div>
                    <h4 className='text-sm font-semibold text-stone-700 mb-1'>Instructions</h4>
                    <p className='text-sm text-stone-600 whitespace-pre-line'>
                      {recipe.instructions}
                    </p>
                  </div>

                  {/* Garnish */}
                  {recipe.garnish && (
                    <p className='text-sm'>
                      <span className='font-semibold text-stone-700'>Garnish:</span>{' '}
                      <span className='text-stone-600'>{recipe.garnish}</span>
                    </p>
                  )}

                  {/* Source URL */}
                  {recipe.source_url && (
                    <p className='text-sm text-stone-500'>
                      Source:{' '}
                      <a
                        href={recipe.source_url}
                        target='_blank'
                        rel='noopener noreferrer'
                        className='text-primary-600 hover:underline'
                      >
                        {(() => {
                          try {
                            return new URL(recipe.source_url).hostname.replace(/^www\./, '')
                          } catch {
                            return recipe.source_url
                          }
                        })()}
                      </a>
                    </p>
                  )}

                  {/* Notes */}
                  {recipe.notes && <p className='text-sm text-stone-500 italic'>{recipe.notes}</p>}

                  {/* Actions */}
                  <div className='flex gap-2 pt-2'>
                    <button
                      onClick={() => startEdit(recipe.id)}
                      className='btn-xs btn-outline'
                    >
                      <IconEdit className='w-3 h-3' />
                      Edit
                    </button>
                    {confirmingDeleteId === recipe.id
                      ? (
                        <span className='flex gap-2 items-center text-sm'>
                          <span className='text-red-600'>Delete?</span>
                          <button
                            onClick={() => handleDelete(recipe.id)}
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
                          onClick={() => setConfirmingDeleteId(recipe.id)}
                          className='btn-xs btn-danger'
                        >
                          <IconTrash className='w-3 h-3' />
                          Delete
                        </button>
                      )}
                  </div>
                </div>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}
