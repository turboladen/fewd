import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ToastProvider, useToast } from './Toast'

/**
 * Harness that exposes the toast() function via a button click. Each click
 * fires toast(message, type) once, so tests can enqueue toasts deterministically.
 */
function Harness({ message, type }: { message: string; type?: 'success' | 'error' | 'info' }) {
  const { toast } = useToast()
  return (
    <button type='button' onClick={() => toast(message, type)}>
      fire:{message}
    </button>
  )
}

function fire(label: string) {
  fireEvent.click(screen.getByRole('button', { name: `fire:${label}` }))
}

describe('Toast', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('useToast outside of a provider is a no-op and does not throw', () => {
    // rendered without <ToastProvider>
    render(<Harness message='orphan' />)
    expect(() => fire('orphan')).not.toThrow()
    // nothing gets rendered
    expect(screen.queryByText('orphan')).toBeNull()
  })

  it('shows a toast message when toast() is called inside the provider', () => {
    render(
      <ToastProvider>
        <Harness message='saved' />
      </ToastProvider>,
    )

    fire('saved')
    expect(screen.getByText('saved')).toBeInTheDocument()
  })

  it('auto-dismisses the toast after 3000ms', () => {
    render(
      <ToastProvider>
        <Harness message='goodbye' />
      </ToastProvider>,
    )

    fire('goodbye')
    expect(screen.getByText('goodbye')).toBeInTheDocument()

    act(() => {
      vi.advanceTimersByTime(3000)
    })

    expect(screen.queryByText('goodbye')).toBeNull()
  })

  it('keeps at most 3 toasts visible, dropping the oldest', () => {
    render(
      <ToastProvider>
        <Harness message='one' />
        <Harness message='two' />
        <Harness message='three' />
        <Harness message='four' />
      </ToastProvider>,
    )

    fire('one')
    fire('two')
    fire('three')
    fire('four')

    // oldest (the one queued before the last 3) is evicted
    expect(screen.queryByText('one')).toBeNull()
    expect(screen.getByText('two')).toBeInTheDocument()
    expect(screen.getByText('three')).toBeInTheDocument()
    expect(screen.getByText('four')).toBeInTheDocument()
  })

  it('dismisses a toast immediately when its X button is clicked', () => {
    render(
      <ToastProvider>
        <Harness message='dismiss-me' />
      </ToastProvider>,
    )

    fire('dismiss-me')
    const toastNode = screen.getByText('dismiss-me').closest('div')!
    const closeBtn = toastNode.querySelector('button')!
    fireEvent.click(closeBtn)

    expect(screen.queryByText('dismiss-me')).toBeNull()

    // advancing past 3000ms must not throw or resurrect the toast
    act(() => {
      vi.advanceTimersByTime(5000)
    })
    expect(screen.queryByText('dismiss-me')).toBeNull()
  })

  it('does not warn about state updates after unmount', () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})

    const { unmount } = render(
      <ToastProvider>
        <Harness message='pending' />
      </ToastProvider>,
    )

    fire('pending')
    unmount()

    act(() => {
      vi.advanceTimersByTime(5000)
    })

    // React warns when setState fires on an unmounted component — the cleanup
    // effect in ToastProvider clears timers, so no warnings should fire.
    const unmountWarnings = errorSpy.mock.calls.filter((c) =>
      typeof c[0] === 'string' && c[0].includes('unmounted')
    )
    expect(unmountWarnings).toHaveLength(0)

    errorSpy.mockRestore()
  })
})
