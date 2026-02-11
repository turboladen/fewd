import { useState } from 'react'
import { IconStar, IconStarFilled } from './Icon'

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

  const sizeClass = size === 'sm' ? 'w-3.5 h-3.5' : 'w-5 h-5'

  if (!interactive && !value) return null

  return (
    <span
      className='inline-flex items-center'
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
            {filled
              ? <IconStarFilled className={sizeClass} />
              : <IconStar className={sizeClass} />}
          </span>
        )
      })}
    </span>
  )
}
