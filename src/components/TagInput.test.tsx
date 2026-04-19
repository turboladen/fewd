import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { TagInput } from './TagInput'

function getTextInput(): HTMLInputElement {
  return screen.getByRole('textbox') as HTMLInputElement
}

describe('TagInput', () => {
  it('renders the label when provided and omits it otherwise', () => {
    const { rerender } = render(
      <TagInput label='Dietary restrictions' value={[]} onChange={() => {}} />,
    )
    expect(screen.getByText('Dietary restrictions')).toBeInTheDocument()

    rerender(<TagInput value={[]} onChange={() => {}} />)
    expect(screen.queryByText('Dietary restrictions')).toBeNull()
  })

  it('uses the default placeholder derived from the label', () => {
    render(<TagInput label='Tags' value={[]} onChange={() => {}} />)
    expect(getTextInput().placeholder).toBe('Add tags (press Enter)')
  })

  it('uses the fallback placeholder when no label is provided', () => {
    render(<TagInput value={[]} onChange={() => {}} />)
    expect(getTextInput().placeholder).toBe('Add item (press Enter)')
  })

  it('honors a custom placeholder over the default', () => {
    render(
      <TagInput
        label='Tags'
        placeholder='Type and press Enter'
        value={[]}
        onChange={() => {}}
      />,
    )
    expect(getTextInput().placeholder).toBe('Type and press Enter')
  })

  it('adds a trimmed tag on Enter and clears the input', () => {
    const onChange = vi.fn()
    render(<TagInput value={['vegan']} onChange={onChange} />)

    const input = getTextInput()
    fireEvent.change(input, { target: { value: '  gluten-free  ' } })
    fireEvent.keyDown(input, { key: 'Enter' })

    expect(onChange).toHaveBeenCalledWith(['vegan', 'gluten-free'])
    expect(input.value).toBe('')
  })

  it('ignores duplicate tags', () => {
    const onChange = vi.fn()
    render(<TagInput value={['vegan']} onChange={onChange} />)

    const input = getTextInput()
    fireEvent.change(input, { target: { value: 'vegan' } })
    fireEvent.keyDown(input, { key: 'Enter' })

    expect(onChange).not.toHaveBeenCalled()
    // input is still cleared, matching the implementation
    expect(input.value).toBe('')
  })

  it('ignores whitespace-only input on Enter', () => {
    const onChange = vi.fn()
    render(<TagInput value={[]} onChange={onChange} />)

    const input = getTextInput()
    fireEvent.change(input, { target: { value: '   ' } })
    fireEvent.keyDown(input, { key: 'Enter' })

    expect(onChange).not.toHaveBeenCalled()
  })

  it('removes the last tag on Backspace when the input is empty', () => {
    const onChange = vi.fn()
    render(<TagInput value={['a', 'b', 'c']} onChange={onChange} />)

    fireEvent.keyDown(getTextInput(), { key: 'Backspace' })
    expect(onChange).toHaveBeenCalledWith(['a', 'b'])
  })

  it('does not remove a tag on Backspace when the input has text', () => {
    const onChange = vi.fn()
    render(<TagInput value={['a', 'b']} onChange={onChange} />)

    const input = getTextInput()
    fireEvent.change(input, { target: { value: 'typing' } })
    fireEvent.keyDown(input, { key: 'Backspace' })

    expect(onChange).not.toHaveBeenCalled()
  })

  it('removes a tag when its remove button is clicked', () => {
    const onChange = vi.fn()
    render(<TagInput value={['a', 'b', 'c']} onChange={onChange} />)

    // tag pills each render a remove <button>; index matches tag order
    const removeButtons = screen.getAllByRole('button')
    fireEvent.click(removeButtons[1])

    expect(onChange).toHaveBeenCalledWith(['a', 'c'])
  })

  it('rerenders tag pills when the controlled value prop changes', () => {
    const { rerender } = render(<TagInput value={['a']} onChange={() => {}} />)
    expect(screen.getByText('a')).toBeInTheDocument()

    rerender(<TagInput value={['x', 'y']} onChange={() => {}} />)
    expect(screen.queryByText('a')).toBeNull()
    expect(screen.getByText('x')).toBeInTheDocument()
    expect(screen.getByText('y')).toBeInTheDocument()
  })
})
