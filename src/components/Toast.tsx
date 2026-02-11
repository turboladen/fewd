import { createContext, useCallback, useContext, useState } from 'react'
import { IconCheck, IconClose, IconWarning } from './Icon'

type ToastType = 'success' | 'error' | 'info'

interface Toast {
  id: number
  message: string
  type: ToastType
}

interface ToastContextValue {
  toast: (message: string, type?: ToastType) => void
}

const ToastContext = createContext<ToastContextValue>({ toast: () => {} })

let nextId = 0

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([])

  const toast = useCallback((message: string, type: ToastType = 'success') => {
    const id = nextId++
    setToasts((prev) => [...prev.slice(-2), { id, message, type }])
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id))
    }, 3000)
  }, [])

  const dismiss = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id))
  }, [])

  return (
    <ToastContext.Provider value={{ toast }}>
      {children}
      {toasts.length > 0 && (
        <div className='fixed bottom-6 left-1/2 -translate-x-1/2 z-50 flex flex-col gap-2 items-center'>
          {toasts.map((t) => (
            <div
              key={t.id}
              className={`animate-slide-up flex items-center gap-2 px-4 py-2.5 rounded-lg shadow-lifted text-sm font-medium ${
                t.type === 'success'
                  ? 'bg-primary-50 border border-primary-200 text-primary-800'
                  : t.type === 'error'
                  ? 'bg-red-50 border border-red-200 text-red-800'
                  : 'bg-stone-50 border border-stone-200 text-stone-800'
              }`}
            >
              {t.type === 'success' && <IconCheck className='w-4 h-4 text-primary-500' />}
              {t.type === 'error' && <IconWarning className='w-4 h-4 text-red-500' />}
              {t.message}
              <button
                onClick={() => dismiss(t.id)}
                className='ml-1 text-stone-400 hover:text-stone-600 transition-colors'
              >
                <IconClose className='w-3.5 h-3.5' />
              </button>
            </div>
          ))}
        </div>
      )}
    </ToastContext.Provider>
  )
}

// eslint-disable-next-line react-refresh/only-export-components
export function useToast() {
  return useContext(ToastContext)
}
