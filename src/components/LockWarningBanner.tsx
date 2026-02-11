import { useState } from 'react'
import { useLockWarning } from '../hooks/useSettings'
import { IconWarning } from './Icon'

export function LockWarningBanner() {
  const { data: lockWarning } = useLockWarning()
  const [dismissed, setDismissed] = useState(false)

  if (!lockWarning || dismissed) return null

  return (
    <div className='flex-none panel-warning border-b px-4 py-2 flex items-center justify-between animate-slide-down'>
      <p className='text-sm text-amber-800 flex items-center gap-1.5'>
        <IconWarning className='w-4 h-4' />
        <span>
          Another computer (<span className='font-medium'>
            &quot;{lockWarning.machine_name}&quot;
          </span>) may be using this database. Simultaneous edits could cause sync issues.
        </span>
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
