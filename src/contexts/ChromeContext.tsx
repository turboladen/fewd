import { createContext, type ReactNode, useContext, useMemo, useState } from 'react'

interface ChromeContextValue {
  isHidden: boolean
  setHidden: (hidden: boolean) => void
}

const ChromeContext = createContext<ChromeContextValue>({
  isHidden: false,
  setHidden: () => {},
})

export function ChromeProvider({ children }: { children: ReactNode }) {
  const [isHidden, setHidden] = useState(false)
  const value = useMemo(() => ({ isHidden, setHidden }), [isHidden])
  return <ChromeContext.Provider value={value}>{children}</ChromeContext.Provider>
}

/**
 * Read and control the global "app chrome" (top nav) visibility.
 * A page can call `setHidden(true)` in an effect to hide the nav for a
 * focused presentation (e.g. cooking mode), and must restore it on cleanup.
 */
// eslint-disable-next-line react-refresh/only-export-components
export function useChrome(): ChromeContextValue {
  return useContext(ChromeContext)
}
