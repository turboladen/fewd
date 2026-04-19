import { fireEvent, render } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { StarRating } from './StarRating'

function interactiveStars(container: HTMLElement): Element[] {
  return Array.from(container.querySelectorAll('span.cursor-pointer'))
}

function filledStars(container: HTMLElement): Element[] {
  return Array.from(container.querySelectorAll('span.text-amber-400'))
}

describe('StarRating', () => {
  it('returns null in read-only mode when value is null', () => {
    const { container } = render(<StarRating value={null} />)
    expect(container.firstChild).toBeNull()
  })

  it('returns null in read-only mode when value is 0', () => {
    const { container } = render(<StarRating value={0} />)
    expect(container.firstChild).toBeNull()
  })

  it('renders 5 stars in read-only mode with the first N filled', () => {
    const { container } = render(<StarRating value={3} />)
    // 5 total stars (each star has one outer <span>, filled ones also match
    // text-amber-400). We check the filled count via the amber class.
    expect(filledStars(container)).toHaveLength(3)
    // no cursor-pointer class when non-interactive
    expect(interactiveStars(container)).toHaveLength(0)
  })

  it('renders 5 interactive empty stars when onChange is provided and value is null', () => {
    const { container } = render(<StarRating value={null} onChange={() => {}} />)
    expect(interactiveStars(container)).toHaveLength(5)
    expect(filledStars(container)).toHaveLength(0)
  })

  it('calls onChange with the clicked star index in interactive mode', () => {
    const onChange = vi.fn()
    const { container } = render(<StarRating value={null} onChange={onChange} />)

    const stars = interactiveStars(container)
    fireEvent.click(stars[3])
    expect(onChange).toHaveBeenCalledWith(4)
  })

  it('previews rating on hover and reverts on mouseleave', () => {
    const { container } = render(<StarRating value={1} onChange={() => {}} />)

    // initial: 1 filled
    expect(filledStars(container)).toHaveLength(1)

    const stars = interactiveStars(container)
    fireEvent.mouseEnter(stars[3]) // 4th star → preview 4 filled
    expect(filledStars(container)).toHaveLength(4)

    // mouseLeave is attached to the wrapper span
    const wrapper = container.querySelector('span.inline-flex')!
    fireEvent.mouseLeave(wrapper)
    expect(filledStars(container)).toHaveLength(1)
  })

  it('does not react to clicks in read-only mode', () => {
    // In read-only mode there are no cursor-pointer stars to click; clicking
    // the raw span elements still must not throw or surface any handler.
    const { container } = render(<StarRating value={3} />)
    const allStars = container.querySelectorAll('span')
    // click every inner span; no handlers should fire — just assert no throw
    allStars.forEach((el) => fireEvent.click(el))
    // still 3 filled
    expect(filledStars(container)).toHaveLength(3)
  })

  it('applies small size class when size="sm"', () => {
    const { container } = render(<StarRating value={5} size='sm' />)
    // size class is applied to the SVG inside each star span
    expect(container.querySelectorAll('svg.w-3\\.5.h-3\\.5')).toHaveLength(5)
  })

  it('applies medium size class by default', () => {
    const { container } = render(<StarRating value={5} />)
    expect(container.querySelectorAll('svg.w-5.h-5')).toHaveLength(5)
  })
})
