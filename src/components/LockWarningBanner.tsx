import { useState } from 'react'
import { useLockWarning } from '../hooks/useSettings'

export function LockWarningBanner() {
  const { data: lockWarning } = useLockWarning()
  const [dismissed, setDismissed] = useState(false)

  if (!lockWarning || dismissed) return null

  return (
    <div className='flex-none bg-amber-50 border-b border-amber-200 px-4 py-2 flex items-center justify-between'>
      <p className='text-sm text-amber-800'>
        {'\u26A0'} Another computer (<span className='font-medium'>
          &quot;{lockWarning.machine_name}&quot;
        </span>) may be using this database. Simultaneous edits could cause sync issues.
      </p>
      <button
        onClick={() => setDismissed(true)}
        className='text-amber-600 hover:text-amber-800 text-sm ml-4 flex-none'
      >
        Dismiss
      </button>
    </div>
  )
}
