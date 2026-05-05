import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { Ingredient } from '../types/recipe'
import { IngredientLineText } from './IngredientLineText'

function ing(name: string, value: number, unit: string, extra?: Partial<Ingredient>): Ingredient {
  return {
    name,
    amount: { type: 'single', value },
    unit,
    notes: undefined,
    ...extra,
  }
}

describe('IngredientLineText', () => {
  it('renders amount, unit, and label inline', () => {
    const { container } = render(
      <IngredientLineText ingredient={ing('flour tortillas', 8, 'whole')} />,
    )
    expect(container.textContent).toBe('8 whole flour tortillas')
  })

  it('appends parenthesized notes when present', () => {
    const { container } = render(
      <IngredientLineText
        ingredient={ing('milk', 1, 'cup', { notes: 'fresh is best' })}
      />,
    )
    expect(container.textContent).toBe('1 cup milk (fresh is best)')
  })

  it('renders an alternative inline with " or " separator', () => {
    const { container } = render(
      <IngredientLineText
        ingredient={{
          ...ing('flour tortillas', 8, 'whole'),
          or_alternative: ing('corn tortillas', 10, 'whole'),
        }}
      />,
    )
    expect(container.textContent).toBe('8 whole flour tortillas or 10 whole corn tortillas')
  })

  it('recurses through chained alternatives', () => {
    const chained: Ingredient = {
      ...ing('milk', 1, 'cup'),
      or_alternative: {
        ...ing('cream', 2, 'cups'),
        or_alternative: ing('water', 3, 'cups'),
      },
    }
    const { container } = render(<IngredientLineText ingredient={chained} />)
    expect(container.textContent).toBe('1 cup milk or 2 cups cream or 3 cups water')
  })
})
