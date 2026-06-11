import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { makeRecipePickerOption } from '../test/factories'
import type { RecipePickerOption } from '../types/recipe'
import { RecipePicker } from './RecipePicker'

// Default fixture covering all three sections:
//   Favorites:      Tacos (Dinner tag), Pancakes (Breakfast tag)
//   Recently used:  Pasta (times_planned > 0)
//   All recipes:    Caesar Salad (Lunch tag), Chicken Curry (lowercase dinner tag)
function makeRecipes(): RecipePickerOption[] {
  return [
    makeRecipePickerOption({ id: 'r-caesar', name: 'Caesar Salad', tags: ['Lunch'] }),
    makeRecipePickerOption({ id: 'r-curry', name: 'Chicken Curry', tags: ['dinner'] }),
    makeRecipePickerOption({
      id: 'r-pancakes',
      name: 'Pancakes',
      tags: ['Breakfast'],
      is_favorite: true,
    }),
    makeRecipePickerOption({
      id: 'r-pasta',
      name: 'Pasta',
      times_planned: 5,
      last_planned: '2026-06-01T00:00:00Z',
    }),
    makeRecipePickerOption({ id: 'r-tacos', name: 'Tacos', tags: ['Dinner'], is_favorite: true }),
  ]
}

function renderPicker(overrides: Partial<Parameters<typeof RecipePicker>[0]> = {}) {
  const onChange = vi.fn()
  render(
    <RecipePicker
      recipes={makeRecipes()}
      value=''
      onChange={onChange}
      {...overrides}
    />,
  )
  return { onChange, input: screen.getByRole('combobox') as HTMLInputElement }
}

function openPicker(input: HTMLInputElement) {
  fireEvent.focus(input)
}

function optionNames(): string[] {
  return screen.getAllByRole('option').map((o) => o.textContent ?? '')
}

describe('RecipePicker — open/close', () => {
  it('starts closed with combobox ARIA wiring', () => {
    const { input } = renderPicker()
    expect(input).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('opens on focus, listing every recipe', () => {
    const { input } = renderPicker()
    openPicker(input)
    expect(input).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('listbox')).toBeInTheDocument()
    expect(screen.getAllByRole('option')).toHaveLength(5)
  })

  it('closes on blur without selecting', () => {
    const { input, onChange } = renderPicker()
    openPicker(input)
    fireEvent.blur(input)
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    expect(onChange).not.toHaveBeenCalled()
  })

  it('shows the selected recipe name when closed', () => {
    const { input } = renderPicker({ value: 'r-pasta' })
    expect(input.value).toBe('Pasta')
  })

  it('falls back to the placeholder when value points at a deleted recipe', () => {
    const { input } = renderPicker({ value: 'r-gone', placeholder: 'Select recipe...' })
    expect(input.value).toBe('')
    expect(input.placeholder).toBe('Select recipe...')
  })
})

describe('RecipePicker — search filtering', () => {
  it('narrows results with a case-insensitive substring match', () => {
    const { input } = renderPicker()
    openPicker(input)
    fireEvent.change(input, { target: { value: 'PAS' } })
    expect(optionNames()).toEqual(['Pasta'])
  })

  it('matches mid-name substrings', () => {
    const { input } = renderPicker()
    openPicker(input)
    fireEvent.change(input, { target: { value: 'urry' } })
    expect(optionNames()).toEqual(['Chicken Curry'])
  })

  it('shows a no-results state when nothing matches', () => {
    const { input } = renderPicker()
    openPicker(input)
    fireEvent.change(input, { target: { value: 'zzz' } })
    expect(screen.queryAllByRole('option')).toHaveLength(0)
    expect(screen.getByText(/no recipes found/i)).toBeInTheDocument()
  })

  it('shows the no-results state for an empty recipe list', () => {
    const { input } = renderPicker({ recipes: [] })
    openPicker(input)
    expect(screen.queryAllByRole('option')).toHaveLength(0)
    expect(screen.getByText(/no recipes found/i)).toBeInTheDocument()
  })
})

describe('RecipePicker — sections & ranking (unfiltered)', () => {
  it('pins favorites on top, then recently used, then all others alphabetically', () => {
    const { input } = renderPicker()
    openPicker(input)
    expect(optionNames()).toEqual([
      'Pancakes',
      'Tacos',
      'Pasta',
      'Caesar Salad',
      'Chicken Curry',
    ])
    expect(screen.getByText('Favorites')).toBeInTheDocument()
    expect(screen.getByText('Recently used')).toBeInTheDocument()
    expect(screen.getByText('All recipes')).toBeInTheDocument()
  })

  it('boosts recipes tagged with the slot meal type within each section (case-insensitive)', () => {
    const { input } = renderPicker({ mealType: 'Dinner' })
    openPicker(input)
    // Favorites: Tacos (Dinner) above Pancakes (Breakfast).
    // All recipes: Chicken Curry ("dinner") above Caesar Salad (Lunch).
    expect(optionNames()).toEqual([
      'Tacos',
      'Pancakes',
      'Pasta',
      'Chicken Curry',
      'Caesar Salad',
    ])
  })

  it('orders recently used by most recent last_planned first', () => {
    const recipes = [
      makeRecipePickerOption({
        id: 'r-old',
        name: 'Old Stew',
        times_planned: 9,
        last_planned: '2026-01-01T00:00:00Z',
      }),
      makeRecipePickerOption({
        id: 'r-new',
        name: 'Zucchini Bake',
        times_planned: 1,
        last_planned: '2026-06-01T00:00:00Z',
      }),
    ]
    const { input } = renderPicker({ recipes })
    openPicker(input)
    expect(optionNames()).toEqual(['Zucchini Bake', 'Old Stew'])
  })
})

describe('RecipePicker — ranking (filtered)', () => {
  it('ranks meal-type tag matches and favorites above plain matches', () => {
    const { input } = renderPicker({ mealType: 'Dinner' })
    openPicker(input)
    fireEvent.change(input, { target: { value: 'a' } })
    // All five names except Chicken Curry contain "a".
    // Tacos: dinner tag + favorite; Pancakes: favorite; Pasta: recently used;
    // Caesar Salad: no boosts.
    expect(optionNames()).toEqual(['Tacos', 'Pancakes', 'Pasta', 'Caesar Salad'])
  })
})

describe('RecipePicker — keyboard support', () => {
  it('moves the highlight with ArrowDown/ArrowUp via aria-activedescendant', () => {
    const { input } = renderPicker()
    openPicker(input)

    const options = screen.getAllByRole('option')
    expect(input).toHaveAttribute('aria-activedescendant', options[0].id)

    fireEvent.keyDown(input, { key: 'ArrowDown' })
    expect(input).toHaveAttribute('aria-activedescendant', options[1].id)

    fireEvent.keyDown(input, { key: 'ArrowUp' })
    expect(input).toHaveAttribute('aria-activedescendant', options[0].id)
  })

  it('does not move the highlight past the ends of the list', () => {
    const { input } = renderPicker({ recipes: [makeRecipePickerOption()] })
    openPicker(input)

    const option = screen.getByRole('option')
    fireEvent.keyDown(input, { key: 'ArrowDown' })
    expect(input).toHaveAttribute('aria-activedescendant', option.id)
    fireEvent.keyDown(input, { key: 'ArrowUp' })
    expect(input).toHaveAttribute('aria-activedescendant', option.id)
  })

  it('selects the highlighted recipe with Enter and closes', () => {
    const { input, onChange } = renderPicker()
    openPicker(input)

    fireEvent.keyDown(input, { key: 'ArrowDown' }) // highlight Tacos (2nd unfiltered)
    fireEvent.keyDown(input, { key: 'Enter' })

    expect(onChange).toHaveBeenCalledWith('r-tacos')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('opens the closed list with ArrowDown', () => {
    const { input } = renderPicker()
    fireEvent.keyDown(input, { key: 'ArrowDown' })
    expect(screen.getByRole('listbox')).toBeInTheDocument()
  })

  it('closes on Escape without selecting and restores the selected name', () => {
    const { input, onChange } = renderPicker({ value: 'r-pasta' })
    openPicker(input)
    fireEvent.change(input, { target: { value: 'tac' } })

    fireEvent.keyDown(input, { key: 'Escape' })

    expect(onChange).not.toHaveBeenCalled()
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    expect(input.value).toBe('Pasta')
  })

  it('starts a fresh query when typing again after Escape closed the list', () => {
    const { input } = renderPicker({ value: 'r-pasta' })
    openPicker(input)
    fireEvent.keyDown(input, { key: 'Escape' })
    // Input still has focus and displays "Pasta"; typing "t" appends to it.
    fireEvent.change(input, { target: { value: 'Pastat' } })

    expect(input.value).toBe('t')
    expect(optionNames()).toEqual(['Tacos', 'Pasta'])
  })

  it('stops Escape from propagating to outer listeners while open', () => {
    const outerListener = vi.fn()
    window.addEventListener('keydown', outerListener)
    try {
      const { input } = renderPicker()
      openPicker(input)
      fireEvent.keyDown(input, { key: 'Escape' })
      expect(outerListener).not.toHaveBeenCalled()

      // A second Escape with the picker closed propagates normally.
      fireEvent.keyDown(input, { key: 'Escape' })
      expect(outerListener).toHaveBeenCalledTimes(1)
    } finally {
      window.removeEventListener('keydown', outerListener)
    }
  })
})

describe('RecipePicker — mouse selection', () => {
  it('selects a recipe on click and closes the list', () => {
    const { input, onChange } = renderPicker()
    openPicker(input)

    fireEvent.click(screen.getByRole('option', { name: 'Chicken Curry' }))

    expect(onChange).toHaveBeenCalledWith('r-curry')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('marks the selected recipe with aria-selected', () => {
    const { input } = renderPicker({ value: 'r-pasta' })
    openPicker(input)
    expect(screen.getByRole('option', { name: 'Pasta' })).toHaveAttribute(
      'aria-selected',
      'true',
    )
    expect(screen.getByRole('option', { name: 'Tacos' })).toHaveAttribute(
      'aria-selected',
      'false',
    )
  })
})
