import { useState } from 'react'

interface StarRatingProps {
  value: number | null
  onChange?: (rating: number) => void
  size?: 'sm' | 'md'
}

export function StarRating({ value, onChange, size = 'md' }: StarRatingProps) {
  const [hoverValue, setHoverValue] = useState<number | null>(null)
  const interactive = !!onChange
  const displayValue = hoverValue ?? value ?? 0
  const stars = [1, 2, 3, 4, 5]

  const sizeClass = size === 'sm' ? 'text-sm' : 'text-lg'

  if (!interactive && !value) return null

  return (
    <span
      className={`inline-flex items-center ${sizeClass}`}
      onMouseLeave={() => interactive && setHoverValue(null)}
    >
      {stars.map((starIndex) => {
        const filled = displayValue >= starIndex
        return (
          <span
            key={starIndex}
            onClick={interactive ? () => onChange?.(starIndex) : undefined}
            onMouseEnter={interactive ? () => setHoverValue(starIndex) : undefined}
            className={`${interactive ? 'cursor-pointer' : ''} ${
              filled ? 'text-amber-400' : 'text-stone-300'
            }`}
          >
            {filled ? '\u2605' : '\u2606'}
          </span>
        )
      })}
    </span>
  )
}
