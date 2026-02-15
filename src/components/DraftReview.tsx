interface DraftReviewProps {
  children: React.ReactNode
  isLoading: boolean
  loadingMessage?: string
  error: string | null
  onAccept: () => void
  onEdit: () => void
  onReject: () => void
  onRegenerate?: () => void
  onCancel?: () => void
  acceptLabel?: string
  editLabel?: string
  rejectLabel?: string
}

export function DraftReview({
  children,
  isLoading,
  loadingMessage,
  error,
  onAccept,
  onEdit,
  onReject,
  onRegenerate,
  onCancel,
  acceptLabel = 'Accept',
  editLabel = 'Edit',
  rejectLabel = 'Reject',
}: DraftReviewProps) {
  if (isLoading) {
    return (
      <div className='panel-primary animate-slide-up'>
        <div className='flex items-center gap-3'>
          <div className='flex gap-1'>
            <div
              className='w-2 h-2 bg-primary-400 rounded-full animate-bounce'
              style={{ animationDelay: '0ms' }}
            />
            <div
              className='w-2 h-2 bg-primary-400 rounded-full animate-bounce'
              style={{ animationDelay: '150ms' }}
            />
            <div
              className='w-2 h-2 bg-primary-400 rounded-full animate-bounce'
              style={{ animationDelay: '300ms' }}
            />
          </div>
          <span className='text-sm text-primary-700'>{loadingMessage || 'Generating...'}</span>
          {onCancel && (
            <button
              onClick={onCancel}
              className='ml-auto text-sm text-stone-500 hover:text-stone-700'
            >
              Cancel
            </button>
          )}
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className='panel-error animate-slide-up'>
        <p className='text-sm text-red-700 mb-2'>{error}</p>
        {onRegenerate && (
          <button
            onClick={onRegenerate}
            className='btn-sm btn-danger-solid'
          >
            Try Again
          </button>
        )}
      </div>
    )
  }

  return (
    <div className='panel-primary animate-fade-in'>
      <div className='mb-3'>{children}</div>
      <div className='flex gap-2'>
        <button
          onClick={onAccept}
          className='btn-sm btn-primary'
        >
          {acceptLabel}
        </button>
        <button
          onClick={onEdit}
          className='btn-sm btn-primary'
        >
          {editLabel}
        </button>
        <button
          onClick={onReject}
          className='btn-sm btn-outline'
        >
          {rejectLabel}
        </button>
        {onRegenerate && (
          <button
            onClick={onRegenerate}
            className='ml-auto text-sm text-stone-500 hover:text-stone-700'
          >
            Regenerate
          </button>
        )}
      </div>
    </div>
  )
}
