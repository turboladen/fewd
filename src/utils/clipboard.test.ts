import { afterEach, describe, expect, it, vi } from 'vitest'
import { copyToClipboard } from './clipboard'

/**
 * These tests pin the secure-context fallback: the async Clipboard API only
 * exists over HTTPS or localhost, so the LAN HTTP deploy (e.g. dietpi.local)
 * must fall back to the legacy execCommand path. See fewd-ejb.
 */

function setSecureContext(value: boolean) {
  Object.defineProperty(window, 'isSecureContext', { value, configurable: true })
}

function setClipboard(value: unknown) {
  Object.defineProperty(navigator, 'clipboard', { value, configurable: true })
}

/** jsdom has no execCommand; install a mock and return it for assertions. */
function setExecCommand(result: boolean) {
  const exec = vi.fn().mockReturnValue(result)
  Object.defineProperty(document, 'execCommand', { value: exec, configurable: true })
  return exec
}

afterEach(() => {
  vi.restoreAllMocks()
})

describe('copyToClipboard', () => {
  it('uses navigator.clipboard.writeText in a secure context', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    setSecureContext(true)
    setClipboard({ writeText })
    const exec = setExecCommand(true)

    await copyToClipboard('secret-token')

    expect(writeText).toHaveBeenCalledWith('secret-token')
    expect(exec).not.toHaveBeenCalled()
  })

  it('falls back to execCommand in an insecure context (LAN HTTP)', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    setSecureContext(false)
    setClipboard({ writeText })
    const exec = setExecCommand(true)

    await copyToClipboard('secret-token')

    expect(writeText).not.toHaveBeenCalled()
    expect(exec).toHaveBeenCalledWith('copy')
  })

  it('falls back to execCommand when the Clipboard API is unavailable', async () => {
    setSecureContext(true)
    setClipboard(undefined)
    const exec = setExecCommand(true)

    await copyToClipboard('secret-token')

    expect(exec).toHaveBeenCalledWith('copy')
  })

  it('falls back to execCommand when writeText rejects', async () => {
    const writeText = vi.fn().mockRejectedValue(new Error('NotAllowedError'))
    setSecureContext(true)
    setClipboard({ writeText })
    const exec = setExecCommand(true)

    await copyToClipboard('secret-token')

    expect(writeText).toHaveBeenCalled()
    expect(exec).toHaveBeenCalledWith('copy')
  })

  it('rejects when the fallback copy fails', async () => {
    setSecureContext(false)
    setClipboard(undefined)
    setExecCommand(false)

    await expect(copyToClipboard('secret-token')).rejects.toThrow()
  })
})
