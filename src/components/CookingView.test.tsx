import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { ChromeProvider, useChrome } from '../contexts/ChromeContext'
import { makeRecipe } from '../test/factories'
import { type ParsedRecipe, parseRecipe } from '../types/recipe'
import { cookingProgressKey, loadCookingProgress } from '../utils/cookingProgress'
import { CookingView } from './CookingView'

afterEach(() => {
  localStorage.clear()
})

function renderCookingView(parsed: ParsedRecipe, onExit = vi.fn()) {
  return {
    onExit,
    ...render(
      <ChromeProvider>
        <CookingView parsed={parsed} onExit={onExit} />
      </ChromeProvider>,
    ),
  }
}

describe('CookingView', () => {
  it('renders the recipe name as the page heading', () => {
    const parsed = parseRecipe(makeRecipe({ name: 'Pasta' }))
    renderCookingView(parsed)
    expect(screen.getByRole('heading', { level: 1, name: 'Pasta' })).toBeInTheDocument()
  })

  it('renders every ingredient and every instruction step', async () => {
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

    // Steps render through the lazy-loaded RecipeMarkdown, so await the first.
    expect(await screen.findByText('Boil water.')).toBeInTheDocument()
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

  it('renders or_alternative ingredients as `primary or alt` in the cooking sidebar', () => {
    // Regression for fewd-2y6.1. The pre-fix CookingView hand-rolled an
    // inline ingredient render that never branched on or_alternative, so a
    // recipe with "8 flour tortillas or 10 corn tortillas" silently showed
    // only the primary while the user was actively cooking.
    const parsed = parseRecipe(makeRecipe({
      ingredients: JSON.stringify([
        {
          name: 'flour tortillas',
          amount: { type: 'single', value: 8 },
          unit: 'whole',
          or_alternative: {
            name: 'corn tortillas',
            amount: { type: 'single', value: 10 },
            unit: 'whole',
          },
        },
      ]),
    }))
    renderCookingView(parsed)

    expect(screen.getByText('flour tortillas')).toBeInTheDocument()
    expect(screen.getByText('corn tortillas')).toBeInTheDocument()
    expect(screen.getByText(/\bor\b/)).toBeInTheDocument()
  })

  it('renders enhancedInstructions in place of parsed.instructions when provided', async () => {
    const parsed = parseRecipe(makeRecipe({
      instructions: 'Original step.',
    }))
    render(
      <ChromeProvider>
        <CookingView
          parsed={parsed}
          onExit={vi.fn()}
          enhancedInstructions={'Enhanced step one.\nEnhanced step two.'}
        />
      </ChromeProvider>,
    )

    expect(await screen.findByText('Enhanced step one.')).toBeInTheDocument()
    expect(screen.queryByText('Original step.')).not.toBeInTheDocument()
    expect(screen.getByText('Enhanced step two.')).toBeInTheDocument()
  })

  it('renders soft-wrapped enhanced paragraphs as one step per paragraph, not per line', async () => {
    const parsed = parseRecipe(makeRecipe())
    const enhanced = [
      'Heat 4 tbsp olive oil in a large pot. Add onion,',
      'carrot, and celery. Cook until soft.',
      '',
      'Add garlic and cook 1 minute more.',
    ].join('\n')

    render(
      <ChromeProvider>
        <CookingView
          parsed={parsed}
          onExit={vi.fn()}
          enhancedInstructions={enhanced}
        />
      </ChromeProvider>,
    )

    // Step bodies render through the lazy RecipeMarkdown, so await one before
    // reading the (synchronously-present) step <li> textContent.
    await screen.findByText('Add garlic and cook 1 minute more.')
    const steps = screen.getAllByRole('listitem').filter((li) => li.closest('ol'))
    expect(steps).toHaveLength(2)
    expect(steps[0].textContent).toContain('Heat 4 tbsp olive oil')
    expect(steps[0].textContent).toContain('Cook until soft.')
    expect(steps[1].textContent).toContain('Add garlic and cook 1 minute more.')
  })

  it('renders **bold** markdown in enhanced instructions as <strong> elements', async () => {
    const parsed = parseRecipe(makeRecipe())
    render(
      <ChromeProvider>
        <CookingView
          parsed={parsed}
          onExit={vi.fn()}
          enhancedInstructions={'Watch the **butter** carefully.'}
        />
      </ChromeProvider>,
    )

    const bold = await screen.findByText('butter')
    expect(bold.tagName).toBe('STRONG')
  })

  it('renders ## section headings as dividers and never leaks literal markers', async () => {
    const parsed = parseRecipe(makeRecipe({
      instructions: [
        '## Caramelized Pineapple',
        '1. Melt butter.',
        '2. Add pineapple.',
        '',
        '## Custard Base',
        '1. Whisk eggs.',
      ].join('\n'),
    }))
    renderCookingView(parsed)

    // Await a markdown-rendered step so the lazy chunk has fully rendered before
    // we assert no literal markers leaked.
    expect(await screen.findByText('Melt butter.')).toBeInTheDocument()

    expect(screen.getByRole('heading', { level: 2, name: 'Caramelized Pineapple' }))
      .toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 2, name: 'Custard Base' })).toBeInTheDocument()

    // Steps render without their markdown markers.
    expect(screen.getByText('Whisk eggs.')).toBeInTheDocument()
  })

  it('hides chrome on mount and restores it on unmount', () => {
    const states: boolean[] = []
    function Probe() {
      const { isHidden } = useChrome()
      states.push(isHidden)
      return null
    }
    const parsed = parseRecipe(makeRecipe())
    function Harness({ mounted }: { mounted: boolean }) {
      return (
        <ChromeProvider>
          <Probe />
          {mounted && <CookingView parsed={parsed} onExit={vi.fn()} />}
        </ChromeProvider>
      )
    }
    const { rerender } = render(<Harness mounted={true} />)
    expect(states.at(-1)).toBe(true)
    rerender(<Harness mounted={false} />)
    expect(states.at(-1)).toBe(false)
  })

  describe('check-off + current-step (fewd-awo)', () => {
    const threeStepRecipe = () =>
      parseRecipe(makeRecipe({
        id: 'cook-1',
        instructions: '1. Boil water.\n2. Add pasta.\n3. Stir.',
        ingredients: JSON.stringify([
          { name: 'Spaghetti', amount: { type: 'single', value: 1 }, unit: 'lb' },
          { name: 'Salt', amount: { type: 'single', value: 1 }, unit: 'tbsp' },
        ]),
      }))

    /** The step toggle button whose body contains `text`. */
    async function stepButton(text: string) {
      const body = await screen.findByText(text)
      const button = body.closest('button')
      if (!button) throw new Error(`no step button wrapping "${text}"`)
      return button
    }

    it('toggles a step between complete and incomplete on click', async () => {
      renderCookingView(threeStepRecipe())
      const step = await stepButton('Add pasta.')

      expect(step).toHaveAttribute('aria-pressed', 'false')
      fireEvent.click(step)
      expect(step).toHaveAttribute('aria-pressed', 'true')
      fireEvent.click(step)
      expect(step).toHaveAttribute('aria-pressed', 'false')
    })

    it('toggles an ingredient between added and not-added on click', () => {
      renderCookingView(threeStepRecipe())
      const ingredient = screen.getByText('Spaghetti').closest('button')!

      expect(ingredient).toHaveAttribute('aria-pressed', 'false')
      fireEvent.click(ingredient)
      expect(ingredient).toHaveAttribute('aria-pressed', 'true')
      fireEvent.click(ingredient)
      expect(ingredient).toHaveAttribute('aria-pressed', 'false')
    })

    it('marks the first incomplete step as the current step', async () => {
      renderCookingView(threeStepRecipe())
      const first = await stepButton('Boil water.')
      const second = await stepButton('Add pasta.')

      expect(first).toHaveAttribute('aria-current', 'step')
      expect(second).not.toHaveAttribute('aria-current')

      // Completing the first advances "current" to the next incomplete step.
      fireEvent.click(first)
      expect(first).not.toHaveAttribute('aria-current')
      expect(second).toHaveAttribute('aria-current', 'step')
    })

    it('restores step + ingredient progress after a remount (mid-cook reload)', async () => {
      const parsed = threeStepRecipe()
      const { unmount } = renderCookingView(parsed)

      fireEvent.click(await stepButton('Boil water.'))
      fireEvent.click(screen.getByText('Salt').closest('button')!)
      unmount()

      renderCookingView(parsed)
      expect(await stepButton('Boil water.')).toHaveAttribute('aria-pressed', 'true')
      expect(screen.getByText('Salt').closest('button')!).toHaveAttribute('aria-pressed', 'true')
      // Current step advanced past the completed first step.
      expect(await stepButton('Add pasta.')).toHaveAttribute('aria-current', 'step')
    })

    it('clears persisted progress when cooking mode is exited', async () => {
      const parsed = threeStepRecipe()
      const { onExit } = renderCookingView(parsed)

      fireEvent.click(await stepButton('Boil water.'))
      expect(loadCookingProgress('cook-1').completedSteps).toEqual([0])
      expect(localStorage.getItem(cookingProgressKey('cook-1'))).not.toBeNull()

      fireEvent.click(screen.getByRole('button', { name: /Exit cooking mode/i }))
      expect(onExit).toHaveBeenCalledTimes(1)
      // The storage key is removed, not just written empty — distinguishing a
      // real clear from a no-op empty write.
      expect(localStorage.getItem(cookingProgressKey('cook-1'))).toBeNull()
    })

    it('does not write a storage entry for a recipe with no check-off activity', () => {
      renderCookingView(threeStepRecipe())
      // Merely viewing cooking mode must not litter localStorage; an entry
      // appears only once the cook checks something off.
      expect(localStorage.getItem(cookingProgressKey('cook-1'))).toBeNull()
    })

    it('does not resurrect a step that was checked then unchecked, after a remount', async () => {
      const parsed = threeStepRecipe()
      const { unmount } = renderCookingView(parsed)

      const first = await stepButton('Boil water.')
      fireEvent.click(first) // check
      fireEvent.click(first) // uncheck — back to empty
      // Empty in-memory state must clear the entry, not leave the stale `[0]`.
      expect(localStorage.getItem(cookingProgressKey('cook-1'))).toBeNull()
      unmount()

      renderCookingView(parsed)
      expect(await stepButton('Boil water.')).toHaveAttribute('aria-pressed', 'false')
    })

    it('tracks step indices globally across section headings', async () => {
      renderCookingView(parseRecipe(makeRecipe({
        id: 'cook-sections',
        instructions: [
          '## Base',
          '1. Melt butter.',
          '',
          '## Top',
          '1. Whisk eggs.',
        ].join('\n'),
      })))

      // Completing the first section's only step makes the second section's
      // step current — proving indices are global, not per-section.
      const melt = await stepButton('Melt butter.')
      expect(melt).toHaveAttribute('aria-current', 'step')
      fireEvent.click(melt)
      expect(await stepButton('Whisk eggs.')).toHaveAttribute('aria-current', 'step')
    })
  })
})
