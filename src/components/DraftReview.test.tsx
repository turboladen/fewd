import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { DraftReview } from './DraftReview'

describe('DraftReview', () => {
  const defaultProps = {
    isLoading: false,
    error: null,
    onAccept: vi.fn(),
    onEdit: vi.fn(),
    onReject: vi.fn(),
  }

  it('renders loading state with generating text', () => {
    render(
      <DraftReview {...defaultProps} isLoading>
        <div>content</div>
      </DraftReview>,
    )
    expect(screen.getByText('Generating...')).toBeInTheDocument()
    expect(screen.queryByText('content')).not.toBeInTheDocument()
  })

  it('renders cancel button during loading when onCancel provided', () => {
    const onCancel = vi.fn()
    render(
      <DraftReview {...defaultProps} isLoading onCancel={onCancel}>
        <div>content</div>
      </DraftReview>,
    )
    const cancelBtn = screen.getByText('Cancel')
    fireEvent.click(cancelBtn)
    expect(onCancel).toHaveBeenCalledOnce()
  })

  it('renders error state with message', () => {
    render(
      <DraftReview {...defaultProps} error='Something went wrong'>
        <div>content</div>
      </DraftReview>,
    )
    expect(screen.getByText('Something went wrong')).toBeInTheDocument()
    expect(screen.queryByText('content')).not.toBeInTheDocument()
  })

  it('renders Try Again button in error state when onRegenerate provided', () => {
    const onRegenerate = vi.fn()
    render(
      <DraftReview {...defaultProps} error='Oops' onRegenerate={onRegenerate}>
        <div>content</div>
      </DraftReview>,
    )
    const retryBtn = screen.getByText('Try Again')
    fireEvent.click(retryBtn)
    expect(onRegenerate).toHaveBeenCalledOnce()
  })

  it('renders review state with children and action buttons', () => {
    render(
      <DraftReview {...defaultProps}>
        <div>AI generated recipe</div>
      </DraftReview>,
    )
    expect(screen.getByText('AI generated recipe')).toBeInTheDocument()
    expect(screen.getByText('Accept')).toBeInTheDocument()
    expect(screen.getByText('Edit')).toBeInTheDocument()
    expect(screen.getByText('Reject')).toBeInTheDocument()
  })

  it('calls onAccept when Accept is clicked', () => {
    const onAccept = vi.fn()
    render(
      <DraftReview {...defaultProps} onAccept={onAccept}>
        <div>content</div>
      </DraftReview>,
    )
    fireEvent.click(screen.getByText('Accept'))
    expect(onAccept).toHaveBeenCalledOnce()
  })

  it('calls onEdit when Edit is clicked', () => {
    const onEdit = vi.fn()
    render(
      <DraftReview {...defaultProps} onEdit={onEdit}>
        <div>content</div>
      </DraftReview>,
    )
    fireEvent.click(screen.getByText('Edit'))
    expect(onEdit).toHaveBeenCalledOnce()
  })

  it('calls onReject when Reject is clicked', () => {
    const onReject = vi.fn()
    render(
      <DraftReview {...defaultProps} onReject={onReject}>
        <div>content</div>
      </DraftReview>,
    )
    fireEvent.click(screen.getByText('Reject'))
    expect(onReject).toHaveBeenCalledOnce()
  })

  it('supports custom button labels', () => {
    render(
      <DraftReview
        {...defaultProps}
        acceptLabel='Save Recipe'
        editLabel='Modify'
        rejectLabel='Discard'
      >
        <div>content</div>
      </DraftReview>,
    )
    expect(screen.getByText('Save Recipe')).toBeInTheDocument()
    expect(screen.getByText('Modify')).toBeInTheDocument()
    expect(screen.getByText('Discard')).toBeInTheDocument()
  })

  it('shows Regenerate button in review state when onRegenerate provided', () => {
    const onRegenerate = vi.fn()
    render(
      <DraftReview {...defaultProps} onRegenerate={onRegenerate}>
        <div>content</div>
      </DraftReview>,
    )
    const regenBtn = screen.getByText('Regenerate')
    fireEvent.click(regenBtn)
    expect(onRegenerate).toHaveBeenCalledOnce()
  })
})
