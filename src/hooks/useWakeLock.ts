import { useEffect } from 'react'

/**
 * Keeps the screen on while `active` is true via the Wake Lock API.
 * Re-acquires after the tab returns from background. No-ops on browsers
 * that don't expose `navigator.wakeLock`. Pass `false` to release without
 * unmounting.
 *
 * Failures are logged but never surfaced — losing wake lock is a soft
 * degradation (the screen dims), not an error worth interrupting the cook.
 */
export function useWakeLock(active: boolean): void {
  useEffect(() => {
    if (!active) return
    if (!('wakeLock' in navigator)) return

    let sentinel: WakeLockSentinel | null = null
    let cancelled = false

    const acquire = async (reason: 'mount' | 'visibility') => {
      try {
        const next = await navigator.wakeLock.request('screen')
        if (cancelled) {
          await next.release().catch(() => {})
          return
        }
        sentinel = next
      } catch (err) {
        console.warn(
          `[useWakeLock] failed to acquire screen wake lock (${reason}):`,
          err,
        )
      }
    }

    const handleVisibilityChange = () => {
      if (document.visibilityState === 'visible' && (!sentinel || sentinel.released)) {
        acquire('visibility')
      }
    }

    acquire('mount')
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
