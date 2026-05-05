import type { Ingredient } from '../types/recipe'
import { formatAmount, formatIngredientLabel } from '../types/recipe'

type Props = {
  ingredient: Ingredient
}

/**
 * Renders one ingredient as inline styled text (amount + unit + label +
 * optional parenthesized notes), recursively appending `or {alternative}`
 * when `or_alternative` is set.
 *
 * Owns only the *text* portion — callers wrap this in their own layout
 * (li, span, fixed-width grid cell). Styling matches the display
 * conventions in RecipeManager and CocktailSuggester so adoption swaps
 * the inline render block for one line of JSX.
 */
export function IngredientLineText({ ingredient }: Props) {
  return (
    <>
      <span className='font-medium'>{formatAmount(ingredient.amount)}</span>
      {ingredient.unit && <span className='text-stone-500'>{` ${ingredient.unit}`}</span>}
      <span>{` ${formatIngredientLabel(ingredient)}`}</span>
      {ingredient.notes && <span className='text-stone-400 italic'>{` (${ingredient.notes})`}
      </span>}
      {ingredient.or_alternative && (
        <>
          <span className='text-stone-500 italic'>{' or '}</span>
          <IngredientLineText ingredient={ingredient.or_alternative} />
        </>
      )}
    </>
  )
}
