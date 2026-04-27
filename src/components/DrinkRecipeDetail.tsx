import type { DrinkRecipe, ParsedDrinkRecipe } from '../types/drinkRecipe'
import { formatAmount, formatIngredientLabel } from '../types/recipe'
import { IconEdit, IconStar, IconStarFilled, IconTrash } from './Icon'
import { StarRating } from './StarRating'

interface Props {
  recipe: DrinkRecipe
  parsed: ParsedDrinkRecipe
  onEdit: () => void
  onToggleFavorite: () => void
  onRatingChange: (rating: number) => void
  onDelete: () => void
  confirmingDelete: boolean
  onConfirmDelete: () => void
  onCancelDelete: () => void
}

export function DrinkRecipeDetail({
  recipe,
  parsed,
  onEdit,
  onToggleFavorite,
  onRatingChange,
  onDelete,
  confirmingDelete,
  onConfirmDelete,
  onCancelDelete,
}: Props) {
  return (
    <div className='card p-6 animate-fade-in'>
      {/* Header */}
      <div className='flex items-start justify-between gap-2'>
        <div className='flex items-center gap-2 min-w-0'>
          <span className='text-2xl flex-shrink-0'>{recipe.icon || '🍸'}</span>
          <div className='min-w-0'>
            <h2 className='text-xl font-semibold text-stone-900'>{recipe.name}</h2>
            {recipe.description && <p className='text-sm text-stone-600'>{recipe.description}</p>}
          </div>
        </div>
        <button
          onClick={onToggleFavorite}
          className='text-amber-500 hover:text-amber-600 flex-shrink-0'
          aria-label={recipe.is_favorite ? 'Unfavorite' : 'Favorite'}
        >
          {recipe.is_favorite
            ? <IconStarFilled className='w-6 h-6' />
            : <IconStar className='w-6 h-6' />}
        </button>
      </div>

      {/* Meta tags */}
      <div className='flex flex-wrap gap-2 mt-3'>
        {recipe.technique && <span className='tag text-xs'>{recipe.technique}</span>}
        {recipe.glassware && <span className='tag text-xs'>{recipe.glassware}</span>}
        {recipe.is_non_alcoholic && (
          <span className='tag text-xs bg-green-100 text-green-700'>non-alcoholic</span>
        )}
        {parsed.tags.map((tag) => <span key={tag} className='tag text-xs'>{tag}</span>)}
      </div>

      {/* Rating */}
      <div className='mt-3'>
        <StarRating
          value={recipe.rating ?? 0}
          onChange={onRatingChange}
          size='md'
        />
      </div>

      <div className='mt-4 pt-4 border-t border-stone-200 space-y-3'>
        {/* Ingredients */}
        <div>
          <h4 className='text-sm font-semibold text-stone-700 mb-1'>Ingredients</h4>
          <ul className='text-sm text-stone-600 space-y-0.5'>
            {parsed.ingredients.map((ing, i) => (
              <li key={i}>
                {formatAmount(ing.amount)} {ing.unit} {formatIngredientLabel(ing)}
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
          <button onClick={onEdit} className='btn-xs btn-outline'>
            <IconEdit className='w-3 h-3' />
            Edit
          </button>
          {confirmingDelete
            ? (
              <span className='flex gap-2 items-center text-sm'>
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
              <button onClick={onDelete} className='btn-xs btn-danger'>
                <IconTrash className='w-3 h-3' />
                Delete
              </button>
            )}
        </div>
      </div>
    </div>
  )
}
