import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { EmptyState } from './EmptyState'

describe('EmptyState', () => {
  it('renders emoji, title, and description', () => {
    render(
      <EmptyState
        emoji='🍅'
        title='No recipes yet'
        description='Add your first recipe to get started.'
      />,
    )

    expect(screen.getByText('🍅')).toBeInTheDocument()
    expect(screen.getByText('No recipes yet')).toBeInTheDocument()
    expect(screen.getByText('Add your first recipe to get started.')).toBeInTheDocument()
  })

  it('omits the action button when no action prop is provided', () => {
    render(
      <EmptyState
        emoji='🍅'
        title='No recipes yet'
        description='Add your first recipe to get started.'
      />,
    )

    expect(screen.queryByRole('button')).toBeNull()
  })

  it('renders the action button with the provided label', () => {
    const onClick = vi.fn()
    render(
      <EmptyState
        emoji='🍅'
        title='No recipes yet'
        description='Add your first recipe to get started.'
        action={{ label: 'Add recipe', onClick }}
      />,
    )

    expect(screen.getByRole('button', { name: 'Add recipe' })).toBeInTheDocument()
  })

  it('fires action.onClick when the action button is clicked', () => {
    const onClick = vi.fn()
    render(
      <EmptyState
        emoji='🍅'
        title='No recipes yet'
        description='Add your first recipe to get started.'
        action={{ label: 'Add recipe', onClick }}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'Add recipe' }))
    expect(onClick).toHaveBeenCalledTimes(1)
  })
})
