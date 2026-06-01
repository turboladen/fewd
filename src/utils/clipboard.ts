/**
 * Copy text to the clipboard, resilient to insecure contexts.
 *
 * The async Clipboard API (`navigator.clipboard`) only exists in a secure
 * context — HTTPS, or `http://localhost`. The LAN HTTP deploy (e.g.
 * `http://dietpi.local`) is insecure, so `navigator.clipboard` is `undefined`
 * there. Fall back to the legacy `execCommand('copy')` path, which works over
 * plain HTTP. Rejects only if both paths fail. See fewd-ejb.
 */
export async function copyToClipboard(text: string): Promise<void> {
  if (window.isSecureContext && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text)
      return
    } catch {
      // Secure context can still reject (e.g. document not focused);
      // fall through to the legacy path rather than giving up.
    }
  }
  if (!copyViaExecCommand(text)) {
    throw new Error('Clipboard copy failed')
  }
}

/** Legacy copy via a transient off-screen textarea + `document.execCommand`. */
function copyViaExecCommand(text: string): boolean {
  const previouslyFocused = document.activeElement
  const textarea = document.createElement('textarea')
  textarea.value = text
  // Keep it out of view and out of the layout/scroll.
  textarea.setAttribute('readonly', '')
  textarea.style.position = 'fixed'
  textarea.style.top = '-9999px'
  document.body.appendChild(textarea)
  try {
    textarea.select()
    // iOS Safari ignores .select() on a readonly field for copy purposes;
    // an explicit range is what makes the selection copyable there.
    textarea.setSelectionRange(0, text.length)
    return document.execCommand('copy')
  } catch {
    return false
  } finally {
    // Always tear down — even if select()/execCommand threw — so we never
    // leak the token-bearing textarea, and hand focus back to the modal.
    document.body.removeChild(textarea)
    if (previouslyFocused instanceof HTMLElement) {
      previouslyFocused.focus()
    }
  }
}
