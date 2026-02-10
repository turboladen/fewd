interface DraftReviewProps {
  children: React.ReactNode
  isLoading: boolean
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
      <div className='border border-primary-200 rounded-lg p-4 bg-primary-50'>
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
          <span className='text-sm text-primary-700'>Generating...</span>
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
      <div className='border border-red-200 rounded-lg p-4 bg-red-50'>
        <p className='text-sm text-red-700 mb-2'>{error}</p>
        {onRegenerate && (
          <button
            onClick={onRegenerate}
            className='text-sm bg-red-600 text-white px-3 py-1 rounded hover:bg-red-700'
          >
            Try Again
          </button>
        )}
      </div>
    )
  }

  return (
    <div className='border border-primary-200 rounded-lg p-4 bg-primary-50'>
      <div className='mb-3'>{children}</div>
      <div className='flex gap-2'>
        <button
          onClick={onAccept}
          className='bg-primary-600 text-white px-3 py-1.5 rounded text-sm hover:bg-primary-700'
        >
          {acceptLabel}
        </button>
        <button
          onClick={onEdit}
          className='bg-primary-600 text-white px-3 py-1.5 rounded text-sm hover:bg-primary-700'
        >
          {editLabel}
        </button>
        <button
          onClick={onReject}
          className='bg-stone-200 text-stone-700 px-3 py-1.5 rounded text-sm hover:bg-stone-300'
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
