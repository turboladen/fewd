import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { makeRecipe } from '../test/factories'
import { type ParsedRecipe, parseRecipe } from '../types/recipe'
import { CookingView } from './CookingView'

function renderCookingView(parsed: ParsedRecipe, onExit = vi.fn()) {
  return { onExit, ...render(<CookingView parsed={parsed} onExit={onExit} />) }
}

describe('CookingView', () => {
  it('renders the recipe name as the page heading', () => {
    const parsed = parseRecipe(makeRecipe({ name: 'Pasta' }))
    renderCookingView(parsed)
    expect(screen.getByRole('heading', { level: 1, name: 'Pasta' })).toBeInTheDocument()
  })

  it('renders every ingredient and every instruction step', () => {
    const parsed = parseRecipe(makeRecipe({
      name: 'Spaghetti aglio e olio',
      ingredients: JSON.stringify([
        { name: 'Spaghetti', amount: { type: 'single', value: 1 }, unit: 'lb' },
        { name: 'Garlic', amount: { type: 'single', value: 6 }, unit: 'cloves' },
      ]),
      instructions: '1. Boil water.\n2. Add pasta.\n3. Stir.',
    }))
    renderCookingView(parsed)

    expect(screen.getByText('Spaghetti')).toBeInTheDocument()
    expect(screen.getByText('Garlic')).toBeInTheDocument()

    expect(screen.getByText('Boil water.')).toBeInTheDocument()
    expect(screen.getByText('Add pasta.')).toBeInTheDocument()
    expect(screen.getByText('Stir.')).toBeInTheDocument()
  })

  it('shows servings and times when present, omits times when not', () => {
    const parsed = parseRecipe(makeRecipe({
      servings: 4,
      prep_time: JSON.stringify({ value: 10, unit: 'minutes' }),
      cook_time: JSON.stringify({ value: 30, unit: 'minutes' }),
      total_time: null,
    }))
    renderCookingView(parsed)

    expect(screen.getByText(/Serves 4/)).toBeInTheDocument()
    expect(screen.getByText(/Prep 10 minutes/)).toBeInTheDocument()
    expect(screen.getByText(/Cook 30 minutes/)).toBeInTheDocument()
    expect(screen.queryByText(/Total/)).not.toBeInTheDocument()
  })

  it('omits the chrome controls that exist on the normal detail view', () => {
    const parsed = parseRecipe(makeRecipe())
    renderCookingView(parsed)

    expect(screen.queryByRole('button', { name: /^Edit$/ })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /Scale/ })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /Adapt/ })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /Delete/ })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /favorites/i })).not.toBeInTheDocument()
  })

  it('renders an Exit cooking mode button that fires onExit when clicked', () => {
    const parsed = parseRecipe(makeRecipe())
    const { onExit } = renderCookingView(parsed)

    const exitButton = screen.getByRole('button', { name: /Exit cooking mode/i })
    expect(exitButton).toBeInTheDocument()

    fireEvent.click(exitButton)
    expect(onExit).toHaveBeenCalledTimes(1)
  })
})
