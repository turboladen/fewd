import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { emptyDrinkRecipeForm } from '../types/drinkRecipe'
import type { Ingredient } from '../types/recipe'
import { DrinkRecipeForm } from './DrinkRecipeForm'

function row(name: string, overrides: Partial<Ingredient> = {}): Ingredient {
  return { name, amount: { type: 'single', value: 1 }, unit: '', notes: undefined, ...overrides }
}

function submit(ingredients: Ingredient[]) {
  const onSubmit = vi.fn()
  render(
    <DrinkRecipeForm
      initialData={{
        ...emptyDrinkRecipeForm,
        name: 'Daiquiri',
        instructions: 'Shake',
        ingredients,
      }}
      onSubmit={onSubmit}
      onCancel={() => {}}
      submitLabel='Save'
    />,
  )
  fireEvent.submit(screen.getByRole('button', { name: 'Save' }).closest('form')!)
  return onSubmit
}

describe('DrinkRecipeForm', () => {
  it('leaves entirely empty rows and alternatives out of the submitted drink', () => {
    const onSubmit = submit([
      row('rum', { unit: 'oz', or_alternative: row('') }),
      row(''),
      row('lime juice'),
    ])

    expect(onSubmit).toHaveBeenCalledTimes(1)
    const sent: Ingredient[] = onSubmit.mock.calls[0][0].ingredients
    expect(sent.map((i) => [i.name, i.or_alternative?.name])).toEqual([
      ['rum', undefined],
      ['lime juice', undefined],
    ])
  })

  it('blocks the save and names a row that has data but no name', () => {
    const onSubmit = submit([row('rum'), row(' ', { notes: 'fresh' })])

    expect(onSubmit).not.toHaveBeenCalled()
    expect(screen.getByText('Ingredient 2 has no name. Name it or remove the row.'))
      .toBeInTheDocument()
  })
})
