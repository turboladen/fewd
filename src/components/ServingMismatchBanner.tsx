interface ServingMismatchBannerProps {
  recipeName: string
  recipeServings: number
  totalPlanned: number
  numPeople: number
  onAdjust: () => void
  onDismiss: () => void
}

export function ServingMismatchBanner({
  recipeName,
  recipeServings,
  totalPlanned,
  numPeople,
  onAdjust,
  onDismiss,
}: ServingMismatchBannerProps) {
  const perPerson = recipeServings / numPeople
  const perPersonLabel = Number.isInteger(perPerson)
    ? perPerson.toString()
    : perPerson.toFixed(1)

  return (
    <div className='bg-amber-50 border border-amber-200 rounded p-3 text-sm'>
      <div className='flex items-start gap-2'>
        <span className='text-amber-500 mt-0.5'>{'\u26A0'}</span>
        <div className='flex-1'>
          <p className='text-amber-800'>
            <span className='font-medium'>{recipeName}</span> makes {recipeServings}{' '}
            servings, but you've planned{' '}
            {Number.isInteger(totalPlanned) ? totalPlanned : totalPlanned.toFixed(1)}.
          </p>
          <div className='mt-2 flex gap-2'>
            <button
              onClick={onAdjust}
              className='text-xs bg-amber-100 border border-amber-300 text-amber-800 px-2 py-1 rounded hover:bg-amber-200'
            >
              Adjust to Full Recipe ({perPersonLabel}/person)
            </button>
            <button
              onClick={onDismiss}
              className='text-xs text-amber-600 hover:text-amber-800 px-2 py-1'
            >
              Dismiss
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
