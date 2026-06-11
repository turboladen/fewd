import { render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { createQueryWrapper } from '../test/queryClient'
import { VersionBadge } from './VersionBadge'

function renderBadge() {
  const { Wrapper } = createQueryWrapper()
  return render(<VersionBadge />, { wrapper: Wrapper })
}

describe('VersionBadge', () => {
  beforeEach(() => {
    installFetchMock()
  })

  afterEach(() => {
    resetFetchMock()
  })

  it('shows version, git sha, and build date once loaded', async () => {
    mockJson('GET', '/api/version', {
      version: '0.1.0',
      git_sha: 'abc123def',
      built_at: '2026-06-11 17:00 UTC',
    })

    renderBadge()

    expect(await screen.findByText(/v0\.1\.0/)).toBeInTheDocument()
    expect(screen.getByText(/abc123def/)).toBeInTheDocument()
    expect(screen.getByText(/2026-06-11 17:00 UTC/)).toBeInTheDocument()
  })

  it('renders nothing when the version fetch fails', async () => {
    mockJson('GET', '/api/version', { message: 'boom' }, { status: 500 })

    const { container } = renderBadge()

    // The badge is a diagnostic nicety — a failed fetch must not add error
    // noise to the Settings panel. Wait for the request to fire (fetch is a
    // vi spy installed by installFetchMock), then assert nothing rendered.
    await waitFor(() => expect(global.fetch).toHaveBeenCalled())
    expect(container).toBeEmptyDOMElement()
  })
})
