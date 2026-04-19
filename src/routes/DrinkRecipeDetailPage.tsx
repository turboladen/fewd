import { useEffect, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { DrinkRecipeDetail } from '../components/DrinkRecipeDetail'
import { DrinkRecipeForm } from '../components/DrinkRecipeForm'
import { EmptyState } from '../components/EmptyState'
import { IconArrowLeft } from '../components/Icon'
import { useToast } from '../components/Toast'
import {
  useDeleteDrinkRecipe,
  useDrinkRecipe,
  useToggleDrinkFavorite,
  useUpdateDrinkRecipe,
} from '../hooks/useDrinkRecipes'
import type { DrinkRecipeFormData, UpdateDrinkRecipeDto } from '../types/drinkRecipe'
import { parseDrinkRecipe } from '../types/drinkRecipe'

export function DrinkRecipeDetailPage() {
  const { id = '' } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const { toast } = useToast()
  const { data: recipe, isLoading, error } = useDrinkRecipe(id)
  const updateMutation = useUpdateDrinkRecipe()
  const deleteMutation = useDeleteDrinkRecipe()
  const toggleFavoriteMutation = useToggleDrinkFavorite()

  const [isEditing, setIsEditing] = useState(false)
  const [confirmingDelete, setConfirmingDelete] = useState(false)

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      if (isEditing) {
        setIsEditing(false)
      } else if (confirmingDelete) {
        setConfirmingDelete(false)
      } else {
        navigate('/cocktails/recipes')
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isEditing, confirmingDelete, navigate])

  if (isLoading) {
    return <div className='p-6 text-stone-500 animate-pulse'>Loading drink recipe...</div>
  }

  if (error || !recipe) {
    return (
      <div className='p-6'>
        <EmptyState
          emoji='🔍'
          title='Drink recipe not found'
          description={error
            ? `Failed to load drink recipe: ${String(error)}`
            : "We couldn't find this drink recipe. It may have been deleted."}
          action={{
            label: 'Back to Drink Recipes',
            onClick: () => navigate('/cocktails/recipes'),
          }}
        />
      </div>
    )
  }

  const parsed = parseDrinkRecipe(recipe)

  const handleUpdate = (formData: DrinkRecipeFormData) => {
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
    updateMutation.mutate({ id: recipe.id, data: dto }, {
      onSuccess: () => {
        setIsEditing(false)
        toast('Drink recipe updated')
      },
    })
  }

  const handleDelete = () => {
    deleteMutation.mutate(recipe.id, {
      onSuccess: () => {
        toast('Drink recipe deleted')
        navigate('/cocktails/recipes')
      },
    })
  }

  const backLink = (
    <Link
      to='/cocktails/recipes'
      className='inline-flex items-center gap-1 text-sm text-stone-500 hover:text-primary-600 mb-4'
    >
      <IconArrowLeft className='w-4 h-4' />
      Back to Drink Recipes
    </Link>
  )

  if (isEditing) {
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
        {backLink}
        <h2 className='text-lg font-semibold text-stone-900 mb-4'>Edit: {recipe.name}</h2>
        <div className='card p-4 animate-slide-up'>
          <DrinkRecipeForm
            initialData={initialData}
            onSubmit={handleUpdate}
            onCancel={() => setIsEditing(false)}
            submitLabel='Save Changes'
          />
        </div>
      </div>
    )
  }

  return (
    <div className='p-6'>
      {backLink}
      <DrinkRecipeDetail
        recipe={recipe}
        parsed={parsed}
        onEdit={() => setIsEditing(true)}
        onToggleFavorite={() => toggleFavoriteMutation.mutate(recipe.id)}
        onRatingChange={(rating) => updateMutation.mutate({ id: recipe.id, data: { rating } })}
        onDelete={() => setConfirmingDelete(true)}
        confirmingDelete={confirmingDelete}
        onConfirmDelete={handleDelete}
        onCancelDelete={() => setConfirmingDelete(false)}
      />
    </div>
  )
}
