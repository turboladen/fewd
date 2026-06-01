import { afterEach, describe, expect, it, vi } from 'vitest'
import { copyToClipboard } from './clipboard'

/**
 * These tests pin the secure-context fallback: the async Clipboard API only
 * exists over HTTPS or localhost, so the LAN HTTP deploy (e.g. dietpi.local)
 * must fall back to the legacy execCommand path. See fewd-ejb.
 */

// `Object.defineProperty` overrides are NOT reverted by vi.restoreAllMocks()
// (that only restores vi.fn/vi.spyOn). Record the original descriptors and
// restore them ourselves so stubs can't leak between tests or files.
const overrides: Array<[object, string, PropertyDescriptor | undefined]> = []

function override(obj: object, prop: string, value: unknown) {
  if (!overrides.some(([o, p]) => o === obj && p === prop)) {
    overrides.push([obj, prop, Object.getOwnPropertyDescriptor(obj, prop)])
  }
  Object.defineProperty(obj, prop, { value, configurable: true })
}

function setSecureContext(value: boolean) {
  override(window, 'isSecureContext', value)
}

function setClipboard(value: unknown) {
  override(navigator, 'clipboard', value)
}

/** jsdom has no execCommand; install a mock and return it for assertions. */
function setExecCommand(result: boolean) {
  const exec = vi.fn().mockReturnValue(result)
  override(document, 'execCommand', exec)
  return exec
}

afterEach(() => {
  for (const [obj, prop, descriptor] of overrides.reverse()) {
    if (descriptor) {
      Object.defineProperty(obj, prop, descriptor)
    } else {
      delete (obj as Record<string, unknown>)[prop]
    }
  }
  overrides.length = 0
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

  it('removes the transient textarea after a successful fallback copy', async () => {
    setSecureContext(false)
    setClipboard(undefined)
    setExecCommand(true)

    await copyToClipboard('secret-token')

    expect(document.querySelector('textarea')).toBeNull()
  })

  it('removes the transient textarea even when the fallback copy fails', async () => {
    setSecureContext(false)
    setClipboard(undefined)
    setExecCommand(false)

    await expect(copyToClipboard('secret-token')).rejects.toThrow()

    expect(document.querySelector('textarea')).toBeNull()
  })
})
