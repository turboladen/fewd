export function FieldToggle({
  label,
  enabled,
  onToggle,
  colorScheme = 'teal',
}: {
  label: string
  enabled: boolean
  onToggle: () => void
  colorScheme?: 'teal' | 'purple'
}) {
  const colors = colorScheme === 'teal'
    ? 'bg-secondary-50 border-secondary-300 text-secondary-700'
    : 'bg-secondary-50 border-secondary-300 text-secondary-700'

  return (
    <button
      onClick={onToggle}
      className={`tag border ${
        enabled
          ? colors
          : 'bg-stone-50 border-stone-200 text-stone-400 line-through'
      }`}
    >
      {label}
    </button>
  )
}

export function PersonSummary({
  goals,
  dislikes,
  favorites,
}: {
  goals: string | null
  dislikes: string[]
  favorites: string[]
}) {
  const parts: string[] = []
  if (goals) parts.push(`Goals: ${goals}`)
  if (dislikes.length > 0) parts.push(`Dislikes: ${dislikes.join(', ')}`)
  if (favorites.length > 0) parts.push(`Favorites: ${favorites.join(', ')}`)

  if (parts.length === 0) return null

  return (
    <p className='text-xs text-stone-500 italic'>
      {parts.join(' | ')}
    </p>
  )
}
