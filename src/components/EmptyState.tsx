interface EmptyStateProps {
  emoji: string
  title: string
  description: string
  action?: {
    label: string
    onClick: () => void
  }
}

export function EmptyState({ emoji, title, description, action }: EmptyStateProps) {
  return (
    <div className='text-center py-16 animate-fade-in'>
      <div className='text-5xl mb-4 opacity-80'>{emoji}</div>
      <h3 className='text-lg font-semibold text-stone-700 mb-2'>{title}</h3>
      <p className='text-sm text-stone-500 max-w-sm mx-auto mb-4'>{description}</p>
      {action && (
        <button onClick={action.onClick} className='btn-md btn-primary'>
          {action.label}
        </button>
      )}
    </div>
  )
}
