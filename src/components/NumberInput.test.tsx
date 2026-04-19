import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { NumberInput } from './NumberInput'

function getInput(): HTMLInputElement {
  return screen.getByRole('spinbutton') as HTMLInputElement
}

describe('NumberInput', () => {
  it('renders the current value as a string', () => {
    render(<NumberInput value={7} onChange={() => {}} />)
    expect(getInput().value).toBe('7')
  })

  it('calls onChange with the parsed number when typing a valid value', () => {
    const onChange = vi.fn()
    render(<NumberInput value={1} onChange={onChange} />)

    fireEvent.change(getInput(), { target: { value: '4' } })
    expect(onChange).toHaveBeenCalledWith(4)
  })

  it('does not call onChange when typing a non-numeric value', () => {
    const onChange = vi.fn()
    render(<NumberInput value={1} onChange={onChange} />)

    // <input type="number"> coerces non-numeric input to '' in the DOM,
    // which parses to NaN — the component must not call onChange.
    fireEvent.change(getInput(), { target: { value: 'abc' } })
    expect(onChange).not.toHaveBeenCalled()
  })

  it('clamps to min on blur when the value is below min', () => {
    const onChange = vi.fn()
    render(<NumberInput value={5} min={3} onChange={onChange} />)

    const input = getInput()
    fireEvent.change(input, { target: { value: '1' } })
    fireEvent.blur(input)

    expect(onChange).toHaveBeenLastCalledWith(3)
    expect(input.value).toBe('3')
  })

  it('clamps to min on blur when the value is blank', () => {
    const onChange = vi.fn()
    render(<NumberInput value={5} min={2} onChange={onChange} />)

    const input = getInput()
    fireEvent.change(input, { target: { value: '' } })
    fireEvent.blur(input)

    expect(onChange).toHaveBeenLastCalledWith(2)
    expect(input.value).toBe('2')
  })

  it('keeps a valid value >= min on blur', () => {
    const onChange = vi.fn()
    render(<NumberInput value={1} min={1} onChange={onChange} />)

    const input = getInput()
    fireEvent.change(input, { target: { value: '7' } })
    fireEvent.blur(input)

    expect(onChange).toHaveBeenLastCalledWith(7)
    expect(input.value).toBe('7')
  })

  it('parses with parseInt when step is undefined (truncates decimals)', () => {
    const onChange = vi.fn()
    render(<NumberInput value={1} onChange={onChange} />)

    fireEvent.change(getInput(), { target: { value: '1.5' } })
    expect(onChange).toHaveBeenCalledWith(1)
  })

  it('parses with parseFloat when step is defined', () => {
    const onChange = vi.fn()
    render(<NumberInput value={1} step='any' onChange={onChange} />)

    fireEvent.change(getInput(), { target: { value: '1.5' } })
    expect(onChange).toHaveBeenCalledWith(1.5)
  })

  it('syncs the displayed value when the value prop changes externally', () => {
    const { rerender } = render(<NumberInput value={2} onChange={() => {}} />)
    expect(getInput().value).toBe('2')

    rerender(<NumberInput value={9} onChange={() => {}} />)
    expect(getInput().value).toBe('9')
  })

  it('forwards required, title, and className props to the input element', () => {
    render(
      <NumberInput
        value={1}
        onChange={() => {}}
        required
        title='Servings'
        className='custom-class'
      />,
    )
    const input = getInput()
    expect(input).toBeRequired()
    expect(input.title).toBe('Servings')
    expect(input.className).toBe('custom-class')
  })
})
