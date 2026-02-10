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
    ? 'bg-teal-50 border-teal-300 text-teal-700'
    : 'bg-purple-50 border-purple-300 text-purple-700'

  return (
    <button
      onClick={onToggle}
      className={`text-xs px-2 py-0.5 rounded border ${
        enabled
          ? colors
          : 'bg-gray-50 border-gray-200 text-gray-400 line-through'
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
    <p className='text-xs text-gray-500 italic'>
      {parts.join(' | ')}
    </p>
  )
}
