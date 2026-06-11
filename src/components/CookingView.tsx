import { useCallback, useEffect } from 'react'
import { useChrome } from '../contexts/ChromeContext'
import { useCookingProgress } from '../hooks/useCookingProgress'
import { useWakeLock } from '../hooks/useWakeLock'
import { formatTime, type ParsedRecipe, parseInstructionSections } from '../types/recipe'
import { fingerprintInstructions } from '../utils/cookingProgress'
import { IconClose } from './Icon'
import { IngredientLineText } from './IngredientLineText'
import { RecipeMarkdown } from './RecipeMarkdown'

interface Props {
  parsed: ParsedRecipe
  onExit: () => void
  /**
   * AI-enhanced instruction text (from `useEnhancedInstructions`). When
   * present and non-empty, displaces `parsed.instructions`; its injected
   * `**amount**` callouts render bold via the shared markdown renderer. Falls
   * back silently when undefined or empty so a missing/failed enhance request
   * never blanks the cook out of their recipe.
   */
  enhancedInstructions?: string
}

export function CookingView({ parsed, onExit, enhancedInstructions }: Props) {
  const sourceText = enhancedInstructions && enhancedInstructions.length > 0
    ? enhancedInstructions
    : parsed.instructions
  const sections = parseInstructionSections(sourceText)
  const hasSteps = sections.some((section) => section.steps.length > 0)
  const totalSteps = sections.reduce((sum, section) => sum + section.steps.length, 0)
  // Scope saved step indices to the exact instruction text they came from, so
  // progress saved against enhanced instructions isn't restored onto the
  // (differently-numbered) original after a hard reload drops the enhancement.
  const fingerprint = fingerprintInstructions(sourceText)
  const { setHidden } = useChrome()

  const {
    isStepComplete,
    isIngredientAdded,
    currentStepIndex,
    toggleStep,
    toggleIngredient,
    reset,
  } = useCookingProgress(parsed.id, totalSteps, fingerprint)

  useEffect(() => {
    setHidden(true)
    return () => setHidden(false)
  }, [setHidden])

  useWakeLock(true)

  // Exiting cooking mode clears progress (a reload, which doesn't run this,
  // restores it). Reset before handing control back to the caller.
  const handleExit = useCallback(() => {
    reset()
    onExit()
  }, [reset, onExit])

  // Escape exits cooking mode through the same path as the Exit button, so
  // every close path runs reset() and honors the exit-clears-progress contract.
  // CookingView owns this (alongside hidden chrome + wake lock); the route's
  // keydown handler no longer special-cases cook mode.
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') handleExit()
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [handleExit])

  // First global step index of each section, so a step's completion +
  // current-step state is tracked across section boundaries.
  const sectionStepBases = sections.reduce<number[]>((bases, _section, i) => {
    bases.push(i === 0 ? 0 : bases[i - 1] + sections[i - 1].steps.length)
    return bases
  }, [])

  return (
    <section className='min-h-screen bg-surface animate-fade-in'>
      <button
        type='button'
        onClick={handleExit}
        aria-label='Exit cooking mode'
        className='btn-ghost fixed top-4 right-4 z-10 inline-flex items-center gap-1.5 bg-white/80 backdrop-blur-xs shadow-soft'
      >
        <IconClose className='w-4 h-4' />
        <span className='hidden sm:inline'>Exit cooking mode</span>
      </button>

      <div className='max-w-5xl mx-auto px-4 sm:px-8 pt-20 pb-12 md:py-16'>
        <header className='mb-10 md:mb-14'>
          <h1 className='font-heading text-4xl md:text-6xl text-stone-900 leading-tight'>
            {parsed.icon && <span className='mr-3'>{parsed.icon}</span>}
            {parsed.name}
          </h1>
          <p className='mt-4 flex flex-wrap gap-x-6 gap-y-1 text-stone-600 text-base md:text-lg'>
            <span>Serves {parsed.servings}</span>
            {parsed.prep_time && <span>Prep {formatTime(parsed.prep_time)}</span>}
            {parsed.cook_time && <span>Cook {formatTime(parsed.cook_time)}</span>}
            {parsed.total_time && <span>Total {formatTime(parsed.total_time)}</span>}
          </p>
        </header>

        <div className='md:grid md:grid-cols-[minmax(220px,30%)_1fr] md:gap-12'>
          <aside className='mb-10 md:mb-0 md:sticky md:top-12 md:self-start md:max-h-[calc(100vh-6rem)] md:overflow-y-auto'>
            <h2 className='font-heading text-2xl md:text-3xl mb-4 text-stone-900'>
              Ingredients
            </h2>
            <ul className='space-y-1 text-lg md:text-base text-stone-700'>
              {parsed.ingredients.map((ing, i) => {
                const added = isIngredientAdded(i)
                return (
                  <li key={i}>
                    <button
                      type='button'
                      aria-pressed={added}
                      onClick={() => toggleIngredient(i)}
                      className={`w-full text-left rounded-md px-2 py-1 -mx-2 transition-colors hover:bg-secondary-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-secondary-500${
                        added ? ' line-through text-stone-400' : ''
                      }`}
                    >
                      <IngredientLineText ingredient={ing} />
                    </button>
                  </li>
                )
              })}
            </ul>
          </aside>

          {hasSteps
            ? (
              <div className='space-y-10 md:space-y-12'>
                {sections.map((section, si) => {
                  const sectionBase = sectionStepBases[si]
                  return (
                    <div key={si}>
                      {section.heading && (
                        <h2 className='font-heading text-2xl md:text-3xl font-semibold text-stone-900 mb-5 md:mb-6'>
                          {section.heading}
                        </h2>
                      )}
                      <ol className='space-y-6 md:space-y-8'>
                        {section.steps.map((step, i) => {
                          const globalIndex = sectionBase + i
                          const completed = isStepComplete(globalIndex)
                          const isCurrent = globalIndex === currentStepIndex
                          return (
                            <li key={i}>
                              <button
                                type='button'
                                aria-pressed={completed}
                                aria-current={isCurrent ? 'step' : undefined}
                                onClick={() => toggleStep(globalIndex)}
                                className={`card w-full text-left p-6 md:p-8 flex gap-4 md:gap-6 items-start transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-secondary-500${
                                  completed ? ' opacity-50' : ''
                                }${
                                  isCurrent
                                    ? ' border-secondary-400 bg-secondary-50 ring-1 ring-secondary-300'
                                    : ''
                                }`}
                              >
                                <span
                                  aria-hidden='true'
                                  className={`font-heading text-5xl md:text-6xl leading-none flex-none tabular-nums${
                                    completed
                                      ? ' text-stone-300 line-through'
                                      : ' text-secondary-600'
                                  }`}
                                >
                                  {globalIndex + 1}
                                </span>
                                <div className='min-w-0 flex-1'>
                                  <RecipeMarkdown markdown={step} variant='cook' />
                                </div>
                              </button>
                            </li>
                          )
                        })}
                      </ol>
                    </div>
                  )
                })}
              </div>
            )
            : (
              <p className='text-stone-400 italic'>
                No instructions for this recipe.
              </p>
            )}
        </div>
      </div>
    </section>
  )
}
