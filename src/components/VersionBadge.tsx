import { useVersion } from '../hooks/useVersion'

/**
 * Build-provenance line for the Settings panel: `v0.1.0 · abc123def · built
 * 2026-06-11 17:00 UTC`. Answers "which build is this server running?" — the
 * 2-second deploy-lag diagnostic (fewd-0vp). Renders nothing until loaded and
 * nothing on error; it must never add noise to Settings.
 */
export function VersionBadge() {
  const { data } = useVersion()

  if (!data) return null

  return (
    <p className='text-xs text-stone-400'>
      v{data.version} · {data.git_sha} · built {data.built_at}
    </p>
  )
}
