import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Ingredient } from '../types/recipe'
import { IngredientInput, IngredientRow } from './IngredientInput'

function single(value: number, name = '', unit = ''): Ingredient {
  return { name, amount: { type: 'single', value }, unit, notes: undefined }
}

function range(min: number, max: number, name = '', unit = ''): Ingredient {
  return { name, amount: { type: 'range', min, max }, unit, notes: undefined }
}

describe('IngredientInput', () => {
  it('renders one row per ingredient in the value prop', () => {
    render(
      <IngredientInput
        value={[single(1, 'flour'), single(2, 'sugar'), single(3, 'salt')]}
        onChange={() => {}}
      />,
    )
    // each row exposes a name input with placeholder 'Ingredient name'
    expect(screen.getAllByPlaceholderText('Ingredient name')).toHaveLength(3)
  })

  it('renders the label when provided and omits it otherwise', () => {
    const { rerender } = render(
      <IngredientInput value={[]} onChange={() => {}} label='Ingredients' />,
    )
    expect(screen.getByText('Ingredients')).toBeInTheDocument()

    rerender(<IngredientInput value={[]} onChange={() => {}} />)
    expect(screen.queryByText('Ingredients')).toBeNull()
  })

  it('appends a default ingredient when "+ Add ingredient" is clicked', () => {
    const onChange = vi.fn()
    render(<IngredientInput value={[single(1, 'flour')]} onChange={onChange} />)

    fireEvent.click(screen.getByRole('button', { name: /add ingredient/i }))

    expect(onChange).toHaveBeenCalledWith([
      single(1, 'flour'),
      { name: '', amount: { type: 'single', value: 1 }, unit: '', notes: undefined },
    ])
  })

  it('removes a row when its trash button is clicked', () => {
    const onChange = vi.fn()
    const { container } = render(
      <IngredientInput
        value={[single(1, 'a'), single(2, 'b'), single(3, 'c')]}
        onChange={onChange}
      />,
    )

    // Each row's trash button is the only `btn-danger` element on that row.
    // Filtering by class avoids tangling with the "+ Add ingredient",
    // "+ Add alternative", and Exact/Range toggle buttons.
    const removeButtons = container.querySelectorAll('button.btn-danger')
    fireEvent.click(removeButtons[1])

    expect(onChange).toHaveBeenCalledWith([single(1, 'a'), single(3, 'c')])
  })

  it('updates the name field via onChange', () => {
    const onChange = vi.fn()
    render(<IngredientInput value={[single(1)]} onChange={onChange} />)

    fireEvent.change(screen.getByPlaceholderText('Ingredient name'), {
      target: { value: 'flour' },
    })

    expect(onChange).toHaveBeenCalledWith([single(1, 'flour')])
  })

  it('updates the unit field via onChange', () => {
    const onChange = vi.fn()
    render(<IngredientInput value={[single(1)]} onChange={onChange} />)

    fireEvent.change(screen.getByPlaceholderText('Unit'), { target: { value: 'cups' } })

    expect(onChange).toHaveBeenCalledWith([single(1, '', 'cups')])
  })

  it('updates the single amount as a float', () => {
    const onChange = vi.fn()
    render(<IngredientInput value={[single(1)]} onChange={onChange} />)

    fireEvent.change(screen.getByPlaceholderText('Amt'), { target: { value: '2.5' } })

    expect(onChange).toHaveBeenCalledWith([
      { name: '', amount: { type: 'single', value: 2.5 }, unit: '', notes: undefined },
    ])
  })

  it('toggles from Exact to Range, promoting single.value to both min and max', () => {
    const onChange = vi.fn()
    render(<IngredientInput value={[single(2)]} onChange={onChange} />)

    fireEvent.click(screen.getByRole('button', { name: 'Exact' }))

    expect(onChange).toHaveBeenCalledWith([
      { name: '', amount: { type: 'range', min: 2, max: 2 }, unit: '', notes: undefined },
    ])
  })

  it('toggles from Range to Exact, preserving the min as the single value', () => {
    const onChange = vi.fn()
    render(<IngredientInput value={[range(2, 4)]} onChange={onChange} />)

    fireEvent.click(screen.getByRole('button', { name: 'Range' }))

    expect(onChange).toHaveBeenCalledWith([
      { name: '', amount: { type: 'single', value: 2 }, unit: '', notes: undefined },
    ])
  })

  it('editing the range min leaves the max untouched (and vice versa)', () => {
    const onChange = vi.fn()
    render(<IngredientInput value={[range(2, 5)]} onChange={onChange} />)

    fireEvent.change(screen.getByPlaceholderText('min'), { target: { value: '3' } })
    expect(onChange).toHaveBeenLastCalledWith([range(3, 5)])

    onChange.mockClear()
    fireEvent.change(screen.getByPlaceholderText('max'), { target: { value: '7' } })
    expect(onChange).toHaveBeenLastCalledWith([range(2, 7)])
  })
})

describe('IngredientRow', () => {
  it('calls onRemove when its trash button is clicked', () => {
    const onRemove = vi.fn()
    const { container } = render(
      <IngredientRow ingredient={single(1, 'flour')} onChange={() => {}} onRemove={onRemove} />,
    )

    // the trash button is the only btn-danger inside the row
    const trash = container.querySelector('button.btn-danger')!
    fireEvent.click(trash)
    expect(onRemove).toHaveBeenCalledTimes(1)
  })

  it('forwards field edits through onChange without mutating the input', () => {
    const onChange = vi.fn()
    const original = single(1, 'flour', 'cups')
    const { container } = render(
      <IngredientRow ingredient={original} onChange={onChange} onRemove={() => {}} />,
    )

    fireEvent.change(within(container).getByPlaceholderText('Ingredient name'), {
      target: { value: 'sugar' },
    })

    expect(onChange).toHaveBeenCalledWith({ ...original, name: 'sugar' })
    // original reference not mutated
    expect(original.name).toBe('flour')
  })

  it('clicking "+ Add alternative" attaches a default Ingredient to or_alternative', () => {
    const onChange = vi.fn()
    const original = single(8, 'flour tortillas', 'whole')
    render(<IngredientRow ingredient={original} onChange={onChange} onRemove={() => {}} />)

    fireEvent.click(screen.getByRole('button', { name: /add alternative/i }))

    expect(onChange).toHaveBeenCalledWith({
      ...original,
      or_alternative: {
        name: '',
        prep: undefined,
        amount: { type: 'single', value: 1 },
        unit: '',
        notes: undefined,
      },
    })
  })

  it('removing the alternative row clears the parent or_alternative without touching primary', () => {
    const onChange = vi.fn()
    const original = {
      ...single(8, 'flour tortillas', 'whole'),
      or_alternative: single(10, 'corn tortillas', 'whole'),
    }
    const { container } = render(
      <IngredientRow ingredient={original} onChange={onChange} onRemove={() => {}} />,
    )

    // Two btn-danger buttons render in the recursive tree — primary first,
    // then nested alt. Clicking the alt's button must clear the parent's
    // or_alternative, not remove the primary.
    const trashButtons = container.querySelectorAll('button.btn-danger')
    expect(trashButtons).toHaveLength(2)
    fireEvent.click(trashButtons[1])

    expect(onChange).toHaveBeenCalledWith({
      ...single(8, 'flour tortillas', 'whole'),
      or_alternative: undefined,
    })
  })
})
