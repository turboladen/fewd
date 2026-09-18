import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { makeRecipe } from '../test/factories'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { createQueryWrapper } from '../test/queryClient'
import { parseRecipe } from '../types/recipe'
import { RecipeDetail } from './RecipeManager'

// A failed "Enhanced view" request keeps the original instructions fully
// visible, surfaces the error, and leaves the toggle out of enhanced mode. A
// successful request enters enhanced mode only when it placed at least one
// amount; otherwise the original instructions stay and a note says so.

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
    mockJson('POST', ENHANCE_URL, { enhanced_text: enhanced, injection_count: 1 })

    renderDetail()

    expect(await screen.findByText(INSTRUCTIONS)).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: /enhanced view/i }))

    // Toggle flips to the enhanced state and the enhanced text replaces the original.
    expect(await screen.findByText(enhanced)).toBeInTheDocument()
    await waitFor(() => expect(screen.queryByText(INSTRUCTIONS)).not.toBeInTheDocument())
    expect(screen.getByRole('button', { name: /^enhanced$/i })).toBeInTheDocument()
    expect(screen.queryByText('No amounts to add')).not.toBeInTheDocument()
  })

  it('stays on the original instructions with a note when no amounts were placed', async () => {
    mockJson('POST', ENHANCE_URL, { enhanced_text: INSTRUCTIONS, injection_count: 0 })
    const enhanceCalls = () =>
      vi.mocked(global.fetch).mock.calls.filter(([input]) => String(input) === ENHANCE_URL).length

    renderDetail()

    expect(await screen.findByText(INSTRUCTIONS)).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: /enhanced view/i }))

    expect(await screen.findByText('No amounts to add')).toBeInTheDocument()
    expect(screen.getByText(INSTRUCTIONS)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /^enhanced$/i })).not.toBeInTheDocument()

    // A second click reuses the cached result instead of asking the server again.
    // A mutation reaches fetch only after a few microtasks, so wait out a timer
    // tick before counting calls, or a refetch would go unseen.
    fireEvent.click(screen.getByRole('button', { name: /enhanced view/i }))
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0))
    })

    expect(screen.getByText('No amounts to add')).toBeInTheDocument()
    expect(screen.getByText(INSTRUCTIONS)).toBeInTheDocument()
    expect(enhanceCalls()).toBe(1)
  })
})
