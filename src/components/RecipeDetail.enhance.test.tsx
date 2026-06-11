import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { makeRecipe } from '../test/factories'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { createQueryWrapper } from '../test/queryClient'
import { parseRecipe } from '../types/recipe'
import { RecipeDetail } from './RecipeManager'

/**
 * Regression coverage for fewd-6kq: a failed "Enhanced view" request must never
 * leave the recipe unviewable. Reported 2026-05-25 — with the server
 * unreachable, clicking the toggle fired a request that failed and the recipe
 * could no longer be shown. The contract pinned here is that the original
 * instructions stay fully visible, an error is surfaced, and the toggle does
 * NOT flip into enhanced mode when the enhance call fails or rejects.
 */

const INSTRUCTIONS = 'Boil water, add pasta.'
const ENHANCE_URL = '/api/recipes/r1/enhance'

function renderDetail() {
  const recipe = makeRecipe({ instructions: INSTRUCTIONS })
  const parsed = parseRecipe(recipe)
  const { Wrapper } = createQueryWrapper()
  const noop = () => {}
  return render(
    <RecipeDetail
      parsed={parsed}
      parentName={null}
      onEdit={noop}
      onScale={noop}
      onAdapt={noop}
      onCook={noop}
      onDelete={noop}
      onToggleFavorite={noop}
      onRatingChange={noop}
      onClose={noop}
      confirmingDelete={false}
      onConfirmDelete={noop}
      onCancelDelete={noop}
    />,
    { wrapper: Wrapper },
  )
}

describe('RecipeDetail enhanced-view failure handling', () => {
  beforeEach(() => {
    installFetchMock()
  })

  afterEach(() => {
    resetFetchMock()
  })

  it('keeps the original instructions visible and stays out of enhanced mode when the enhance POST fails (500)', async () => {
    mockJson('POST', ENHANCE_URL, { message: 'AI service unavailable' }, { status: 500 })

    renderDetail()

    // Instructions render through the lazy markdown chunk — await its resolution.
    expect(await screen.findByText(INSTRUCTIONS)).toBeInTheDocument()

    const toggle = screen.getByRole('button', { name: /enhanced view/i })
    fireEvent.click(toggle)

    // An error indication appears once the request fails.
    expect(await screen.findByText(/AI service unavailable/)).toBeInTheDocument()

    // Original instructions are still fully visible.
    expect(screen.getByText(INSTRUCTIONS)).toBeInTheDocument()

    // The toggle did NOT flip into enhanced mode (no "Enhanced ✓" state).
    expect(screen.getByRole('button', { name: /enhanced view/i })).toBeInTheDocument()
  })

  it('keeps the recipe viewable when the enhance request rejects (network error)', async () => {
    renderDetail()

    expect(await screen.findByText(INSTRUCTIONS)).toBeInTheDocument()

    // Simulate the reported scenario: server unreachable, fetch rejects.
    vi.spyOn(global, 'fetch').mockRejectedValueOnce(new TypeError('Failed to fetch'))

    const toggle = screen.getByRole('button', { name: /enhanced view/i })
    fireEvent.click(toggle)

    // Error is surfaced and the original instructions remain visible.
    expect(await screen.findByText(/Failed to fetch/)).toBeInTheDocument()
    expect(screen.getByText(INSTRUCTIONS)).toBeInTheDocument()

    // Still not flipped into enhanced mode — the recipe stays viewable.
    expect(screen.getByRole('button', { name: /enhanced view/i })).toBeInTheDocument()
  })

  it('swaps in enhanced text only on success', async () => {
    const enhanced = 'Step 1: Bring a large pot of salted water to a rolling boil.'
    mockJson('POST', ENHANCE_URL, enhanced)

    renderDetail()

    expect(await screen.findByText(INSTRUCTIONS)).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: /enhanced view/i }))

    // Toggle flips to the enhanced state and the enhanced text replaces the original.
    expect(await screen.findByText(enhanced)).toBeInTheDocument()
    await waitFor(() => expect(screen.queryByText(INSTRUCTIONS)).not.toBeInTheDocument())
  })
})
