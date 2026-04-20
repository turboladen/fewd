import { createContext, type ReactNode, useContext, useMemo, useState } from 'react'

interface ChromeContextValue {
  isHidden: boolean
  setHidden: (hidden: boolean) => void
}

const ChromeContext = createContext<ChromeContextValue | null>(null)

export function ChromeProvider({ children }: { children: ReactNode }) {
  const [isHidden, setHidden] = useState(false)
  const value = useMemo(() => ({ isHidden, setHidden }), [isHidden])
  return <ChromeContext.Provider value={value}>{children}</ChromeContext.Provider>
}

/**
 * Read and control whether the app chrome is rendered.
 *
 * Lives in context so any descendant route can opt out of chrome without
 * the layout needing to know about it. Callers are expected to call
 * `setHidden(true)` from an effect on mount and `setHidden(false)` from
 * the cleanup — there is no ref-counting, so two simultaneous owners
 * would stomp on each other's state.
 */
// eslint-disable-next-line react-refresh/only-export-components
export function useChrome(): ChromeContextValue {
  const ctx = useContext(ChromeContext)
  if (!ctx) {
    throw new Error('useChrome must be used inside <ChromeProvider>')
  }
  return ctx
}
