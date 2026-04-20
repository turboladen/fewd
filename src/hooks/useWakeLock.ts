import { useEffect } from 'react'

interface WakeLockSentinelLike {
  release(): Promise<void>
  released: boolean
}

interface NavigatorWithWakeLock extends Navigator {
  wakeLock?: {
    request(type: 'screen'): Promise<WakeLockSentinelLike>
  }
}

/**
 * Keeps the screen on while `active` is true, using the Wake Lock API.
 * Re-acquires after the tab returns from background. No-ops on browsers
 * that don't expose `navigator.wakeLock` (e.g. Safari desktop).
 *
 * Failures are logged, never surfaced — losing wake lock is a soft
 * degradation, not an error worth interrupting the cook.
 */
export function useWakeLock(active: boolean): void {
  useEffect(() => {
    if (!active) return

    const nav = navigator as NavigatorWithWakeLock
    if (!nav.wakeLock) return

    let sentinel: WakeLockSentinelLike | null = null
    let cancelled = false

    const acquire = async () => {
      try {
        const next = await nav.wakeLock!.request('screen')
        if (cancelled) {
          await next.release().catch(() => {})
          return
        }
        sentinel = next
      } catch (err) {
        console.warn('[useWakeLock] failed to acquire screen wake lock:', err)
      }
    }

    const handleVisibilityChange = () => {
      if (document.visibilityState === 'visible' && (!sentinel || sentinel.released)) {
        acquire()
      }
    }

    acquire()
    document.addEventListener('visibilitychange', handleVisibilityChange)

    return () => {
      cancelled = true
      document.removeEventListener('visibilitychange', handleVisibilityChange)
      if (sentinel && !sentinel.released) {
        sentinel.release().catch(() => {})
      }
    }
  }, [active])
}
