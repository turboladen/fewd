import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { ServingMismatchBanner } from './ServingMismatchBanner'

describe('ServingMismatchBanner', () => {
  const defaultProps = {
    recipeName: 'Spaghetti Bolognese',
    recipeServings: 4,
    totalPlanned: 2,
    numPeople: 2,
    onAdjust: vi.fn(),
    onDismiss: vi.fn(),
  }

  it('renders recipe name and serving counts', () => {
    render(<ServingMismatchBanner {...defaultProps} />)
    expect(screen.getByText('Spaghetti Bolognese')).toBeTruthy()
    expect(screen.getByText(/makes 4/)).toBeTruthy()
    expect(screen.getByText(/planned 2/)).toBeTruthy()
  })

  it('shows per-person adjusted servings in button', () => {
    render(<ServingMismatchBanner {...defaultProps} />)
    // 4 servings / 2 people = 2/person
    expect(screen.getByText(/Adjust to Full Recipe \(2\/person\)/)).toBeTruthy()
  })

  it('shows fractional per-person amount when not even', () => {
    render(<ServingMismatchBanner {...defaultProps} recipeServings={3} numPeople={2} />)
    // 3 servings / 2 people = 1.5/person
    expect(screen.getByText(/1\.5\/person/)).toBeTruthy()
  })

  it('calls onAdjust when adjust button clicked', () => {
    const onAdjust = vi.fn()
    render(<ServingMismatchBanner {...defaultProps} onAdjust={onAdjust} />)
    fireEvent.click(screen.getByText(/Adjust to Full Recipe/))
    expect(onAdjust).toHaveBeenCalledOnce()
  })

  it('calls onDismiss when dismiss button clicked', () => {
    const onDismiss = vi.fn()
    render(<ServingMismatchBanner {...defaultProps} onDismiss={onDismiss} />)
    fireEvent.click(screen.getByText('Dismiss'))
    expect(onDismiss).toHaveBeenCalledOnce()
  })
})
