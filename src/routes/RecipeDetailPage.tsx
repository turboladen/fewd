import { useEffect, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { AdaptRecipePanel } from '../components/AdaptRecipePanel'
import { EmptyState } from '../components/EmptyState'
import { IconArrowLeft } from '../components/Icon'
import {
  RecipeDetail,
  RecipeForm,
  type RecipeFormData,
  ScaleRecipePanel,
} from '../components/RecipeManager'
import { useToast } from '../components/Toast'
import {
  useCreateRecipe,
  useDeleteRecipe,
  useRecipe,
  useToggleFavorite,
  useUpdateRecipe,
} from '../hooks/useRecipes'
import type { CreateRecipeDto, Ingredient, UpdateRecipeDto } from '../types/recipe'
import { parseRecipe } from '../types/recipe'

type Mode = 'view' | 'edit' | 'scale' | 'adapt' | 'adapt-edit'

export function RecipeDetailPage() {
  const { id } = useParams<{ id: string }>()
  if (!id) throw new Error('RecipeDetailPage rendered without :id param')

  const navigate = useNavigate()
  const { toast } = useToast()
  const { data: recipe, isLoading, error } = useRecipe(id)
  const { data: parentRecipe } = useRecipe(recipe?.parent_recipe_id ?? '')
  const createMutation = useCreateRecipe()
  const updateMutation = useUpdateRecipe()
  const deleteMutation = useDeleteRecipe()
  const toggleFavoriteMutation = useToggleFavorite()

  const [mode, setMode] = useState<Mode>('view')
  const [confirmingDelete, setConfirmingDelete] = useState(false)
  const [adaptDraft, setAdaptDraft] = useState<CreateRecipeDto | null>(null)
  const [scaleError, setScaleError] = useState<string | null>(null)

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      if (mode !== 'view') {
        setMode('view')
        setAdaptDraft(null)
        return
      }
      if (confirmingDelete) {
        setConfirmingDelete(false)
        return
      }
      navigate('/recipes')
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [mode, confirmingDelete, navigate])

  if (isLoading) {
    return <div className='p-6 text-stone-500 animate-pulse'>Loading recipe...</div>
  }

  if (error || !recipe) {
    return (
      <div className='p-6'>
        <EmptyState
          emoji='🔍'
          title='Recipe not found'
          description={error
            ? `Failed to load recipe: ${String(error)}`
            : "We couldn't find this recipe. It may have been deleted."}
          action={{ label: 'Back to Recipes', onClick: () => navigate('/recipes') }}
        />
      </div>
    )
  }

  const parsed = parseRecipe(recipe)
  const parentName = parentRecipe?.name ?? null

  const handleUpdate = (formData: RecipeFormData) => {
    const dto: UpdateRecipeDto = {
      name: formData.name,
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
    updateMutation.mutate({ id: recipe.id, data: dto }, {
      onSuccess: () => {
        toast('Recipe updated')
        setMode('view')
      },
    })
  }

  const handleDelete = () => {
    deleteMutation.mutate(recipe.id, {
      onSuccess: () => {
        toast('Recipe deleted')
        navigate('/recipes')
      },
    })
  }

  const handleScaleSaveAsNew = (ingredients: Ingredient[], servings: number) => {
    const dto: CreateRecipeDto = {
      name: `${recipe.name} (${servings} servings)`,
      source: 'scaled',
      parent_recipe_id: recipe.id,
      servings,
      portion_size: parsed.portion_size ?? undefined,
      instructions: recipe.instructions,
      ingredients,
      tags: parsed.tags,
      description: recipe.description || undefined,
      prep_time: parsed.prep_time ?? undefined,
      cook_time: parsed.cook_time ?? undefined,
      total_time: parsed.total_time ?? undefined,
      notes: recipe.notes || undefined,
      icon: recipe.icon || undefined,
    }
    setScaleError(null)
    createMutation.mutate(dto, {
      onSuccess: (created) => {
        setMode('view')
        navigate(`/recipes/${created.id}`)
      },
      onError: (err) => setScaleError(String(err)),
    })
  }

  const handleScaleUpdateInPlace = (ingredients: Ingredient[], servings: number) => {
    const dto: UpdateRecipeDto = { ingredients, servings }
    setScaleError(null)
    updateMutation.mutate({ id: recipe.id, data: dto }, {
      onSuccess: () => setMode('view'),
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
      onSuccess: (created) => {
        setMode('view')
        setAdaptDraft(null)
        navigate(`/recipes/${created.id}`)
      },
    })
  }

  const backLink = (
    <Link
      to='/recipes'
      className='inline-flex items-center gap-1 text-sm text-stone-500 hover:text-primary-600 mb-4'
    >
      <IconArrowLeft className='w-4 h-4' />
      Back to Recipes
    </Link>
  )

  if (mode === 'edit' || mode === 'adapt-edit') {
    const isAdaptEdit = mode === 'adapt-edit' && !!adaptDraft
    const formInitial: RecipeFormData = isAdaptEdit
      ? {
        name: adaptDraft!.name,
        description: adaptDraft!.description || '',
        prep_time: adaptDraft!.prep_time,
        cook_time: adaptDraft!.cook_time,
        total_time: adaptDraft!.total_time,
        servings: adaptDraft!.servings,
        portion_size: adaptDraft!.portion_size,
        instructions: adaptDraft!.instructions,
        ingredients: adaptDraft!.ingredients,
        tags: adaptDraft!.tags,
        notes: adaptDraft!.notes || '',
        icon: adaptDraft!.icon || '',
      }
      : {
        name: recipe.name,
        description: recipe.description || '',
        prep_time: parsed.prep_time ?? undefined,
        cook_time: parsed.cook_time ?? undefined,
        total_time: parsed.total_time ?? undefined,
        servings: recipe.servings,
        portion_size: parsed.portion_size ?? undefined,
        instructions: recipe.instructions,
        ingredients: parsed.ingredients,
        tags: parsed.tags,
        notes: recipe.notes || '',
        icon: recipe.icon || '',
      }
    return (
      <div className='p-6'>
        {backLink}
        <div
          className={`animate-slide-up ${isAdaptEdit ? 'panel-secondary' : 'panel-primary'}`}
        >
          <h3 className='font-semibold text-lg mb-3'>
            {isAdaptEdit ? 'Edit Adapted Recipe' : `Edit ${recipe.name}`}
          </h3>
          <RecipeForm
            initialData={formInitial}
            onSubmit={isAdaptEdit ? handleAdaptDraftSave : handleUpdate}
            onCancel={() => {
              setMode('view')
              setAdaptDraft(null)
            }}
            submitLabel={isAdaptEdit ? 'Save Adapted Recipe' : 'Save Changes'}
          />
          {(isAdaptEdit ? createMutation.error : updateMutation.error) && (
            <div className='mt-2 panel-error text-red-700 text-sm'>
              {String(isAdaptEdit ? createMutation.error : updateMutation.error)}
            </div>
          )}
        </div>
      </div>
    )
  }

  if (mode === 'scale') {
    return (
      <div className='p-6'>
        {backLink}
        <ScaleRecipePanel
          parsed={parsed}
          onSaveAsNew={handleScaleSaveAsNew}
          onUpdateInPlace={handleScaleUpdateInPlace}
          onCancel={() => {
            setMode('view')
            setScaleError(null)
          }}
          error={scaleError ?? undefined}
        />
      </div>
    )
  }

  if (mode === 'adapt') {
    return (
      <div className='p-6'>
        {backLink}
        <AdaptRecipePanel
          parsed={parsed}
          onComplete={(newId) => {
            setMode('view')
            navigate(`/recipes/${newId}`)
          }}
          onEdit={(draft) => {
            setAdaptDraft(draft)
            setMode('adapt-edit')
          }}
          onCancel={() => setMode('view')}
        />
      </div>
    )
  }

  return (
    <div className='p-6'>
      {backLink}
      <div className='grid grid-cols-1 md:grid-cols-2 gap-4'>
        <RecipeDetail
          parsed={parsed}
          parentName={parentName}
          onEdit={() => setMode('edit')}
          onScale={() => setMode('scale')}
          onAdapt={() => setMode('adapt')}
          onDelete={() => setConfirmingDelete(true)}
          onToggleFavorite={() => toggleFavoriteMutation.mutate(recipe.id)}
          onRatingChange={(rating) => updateMutation.mutate({ id: recipe.id, data: { rating } })}
          onClose={() => navigate('/recipes')}
          confirmingDelete={confirmingDelete}
          onConfirmDelete={handleDelete}
          onCancelDelete={() => setConfirmingDelete(false)}
        />
      </div>
    </div>
  )
}
