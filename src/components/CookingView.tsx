import { useEffect } from 'react'
import { useChrome } from '../contexts/ChromeContext'
import { useWakeLock } from '../hooks/useWakeLock'
import { formatAmount, formatTime, type ParsedRecipe, parseInstructionSteps } from '../types/recipe'
import { IconClose } from './Icon'

interface Props {
  parsed: ParsedRecipe
  onExit: () => void
}

export function CookingView({ parsed, onExit }: Props) {
  const steps = parseInstructionSteps(parsed.instructions)
  const { setHidden } = useChrome()

  useEffect(() => {
    setHidden(true)
    return () => setHidden(false)
  }, [setHidden])

  useWakeLock(true)

  return (
    <section className='min-h-screen bg-surface animate-fade-in'>
      <button
        type='button'
        onClick={onExit}
        aria-label='Exit cooking mode'
        className='btn-ghost fixed top-4 right-4 z-10 inline-flex items-center gap-1.5 bg-white/80 backdrop-blur-sm shadow-soft'
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
            <ul className='space-y-2 text-lg md:text-base text-stone-700'>
              {parsed.ingredients.map((ing, i) => (
                <li key={i}>
                  <span className='font-semibold text-stone-900'>
                    {formatAmount(ing.amount)}
                  </span>
                  {ing.unit && <span className='text-stone-500'>{` ${ing.unit}`}</span>}
                  <span>{` ${ing.name}`}</span>
                  {ing.notes && <span className='text-stone-400 italic'>{` (${ing.notes})`}</span>}
                </li>
              ))}
            </ul>
          </aside>

          {steps.length > 0
            ? (
              <ol className='space-y-6 md:space-y-8'>
                {steps.map((step, i) => (
                  <li
                    key={i}
                    className='card p-6 md:p-8 flex gap-4 md:gap-6 items-start'
                  >
                    <span
                      aria-hidden='true'
                      className='font-heading text-secondary-600 text-5xl md:text-6xl leading-none flex-none tabular-nums'
                    >
                      {i + 1}
                    </span>
                    <p className='text-lg md:text-xl leading-relaxed text-stone-800'>
                      {step}
                    </p>
                  </li>
                ))}
              </ol>
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
