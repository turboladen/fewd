import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ErrorBoundary } from './ErrorBoundary'

function Bomb({ message = 'kaboom' }: { message?: string }) {
  throw new Error(message)
}

describe('ErrorBoundary', () => {
  let errorSpy: ReturnType<typeof vi.spyOn>

  beforeEach(() => {
    errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
  })

  afterEach(() => {
    errorSpy.mockRestore()
  })

  it('renders children when no error is thrown', () => {
    render(
      <ErrorBoundary>
        <p>safe child</p>
      </ErrorBoundary>,
    )
    expect(screen.getByText('safe child')).toBeTruthy()
    expect(screen.queryByText('Something went wrong')).toBeNull()
  })

  it('renders fallback UI when a child throws', () => {
    render(
      <ErrorBoundary>
        <Bomb message='boundary caught me' />
      </ErrorBoundary>,
    )
    expect(screen.getByText('Something went wrong')).toBeTruthy()
    expect(screen.getByText('boundary caught me')).toBeTruthy()
    expect(screen.getByRole('button', { name: /try again/i })).toBeTruthy()
    expect(screen.getByRole('button', { name: /reload app/i })).toBeTruthy()
  })

  it('logs the caught error via console.error', () => {
    render(
      <ErrorBoundary>
        <Bomb />
      </ErrorBoundary>,
    )
    const loggedOurMessage = errorSpy.mock.calls.some(call =>
      call.some(arg => arg === 'Uncaught error:')
    )
    expect(loggedOurMessage).toBe(true)
  })

  it('clears the fallback and renders new children after reset', () => {
    const { rerender } = render(
      <ErrorBoundary>
        <Bomb />
      </ErrorBoundary>,
    )
    expect(screen.getByText('Something went wrong')).toBeTruthy()

    // Rerender with safe children BEFORE clicking reset — otherwise the
    // reset re-renders the still-throwing Bomb and bounces hasError back
    // to true. This models the real flow: the underlying cause must be
    // gone before "Try Again" can succeed.
    rerender(
      <ErrorBoundary>
        <p>recovered child</p>
      </ErrorBoundary>,
    )
    expect(screen.getByText('Something went wrong')).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: /try again/i }))

    expect(screen.queryByText('Something went wrong')).toBeNull()
    expect(screen.getByText('recovered child')).toBeTruthy()
  })
})
