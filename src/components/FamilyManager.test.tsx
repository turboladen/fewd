import { fireEvent, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { makePerson } from '../test/factories'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { renderWithProviders } from '../test/renderWithProviders'
import { FamilyManager } from './FamilyManager'

beforeEach(() => installFetchMock())
afterEach(() => resetFetchMock())

// PersonForm labels aren't wired to inputs via htmlFor/id, so locate fields
// positionally via getAllByRole (textboxes) plus a container query for the
// date input (which isn't role=textbox in JSDOM).
function fillRequiredFields(
  container: HTMLElement,
  { name, birthdate }: { name: string; birthdate: string },
) {
  const nameInput = screen.getAllByRole('textbox')[0]
  fireEvent.change(nameInput, { target: { value: name } })
  const dateInput = container.querySelector<HTMLInputElement>('input[type="date"]')!
  fireEvent.change(dateInput, { target: { value: birthdate } })
}

describe('FamilyManager', () => {
  it('shows the empty state when no family members exist', async () => {
    mockJson('GET', '/api/people', [])

    renderWithProviders(<FamilyManager />)

    await waitFor(() => {
      expect(screen.getByText('Your family awaits')).toBeInTheDocument()
    })
    // Empty-state description confirms no cards rendered.
    expect(
      screen.getByText('Add your first family member to start planning meals together.'),
    ).toBeInTheDocument()
  })

  it('renders the list of family members', async () => {
    mockJson('GET', '/api/people', [
      makePerson({ id: 'p1', name: 'Alice' }),
      makePerson({ id: 'p2', name: 'Bob' }),
    ])

    renderWithProviders(<FamilyManager />)

    await waitFor(() => expect(screen.getByText('Alice')).toBeInTheDocument())
    expect(screen.getByText('Bob')).toBeInTheDocument()
  })

  it('adds a person: form submit → POST → list refetches and new card appears', async () => {
    mockJson('GET', '/api/people', [])

    const { container, client } = renderWithProviders(<FamilyManager />)
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    await waitFor(() => expect(screen.getByText('Your family awaits')).toBeInTheDocument())

    // Empty list renders BOTH the header trigger and the empty-state action
    // with the same accessible name. Pick the first (header) to open the form —
    // once isAdding flips, both triggers unmount and only the form's submit
    // button with this name remains.
    fireEvent.click(screen.getAllByRole('button', { name: 'Add Person' })[0])

    fillRequiredFields(container, { name: 'Carol', birthdate: '2020-05-01' })

    // Stage the POST + the post-invalidation list refetch (last-match-wins).
    const created = makePerson({ id: 'p-new', name: 'Carol', birthdate: '2020-05-01' })
    mockJson('POST', '/api/people', created)
    mockJson('GET', '/api/people', [created])

    // Submit the form — now the only 'Add Person' button in the DOM.
    fireEvent.click(screen.getByRole('button', { name: 'Add Person' }))

    // Behavior: card appears after refetch.
    await waitFor(() => expect(screen.getByText('Carol')).toBeInTheDocument())

    // Contract: invalidation targets the ['people'] key.
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['people'] })

    // Behavior: POST body carried the submitted fields.
    const postCall = vi.mocked(fetch).mock.calls.find(([, init]) =>
      (init as RequestInit | undefined)?.method === 'POST'
    )
    expect(postCall).toBeDefined()
    const postBody = JSON.parse((postCall![1] as RequestInit).body as string)
    expect(postBody.name).toBe('Carol')
    expect(postBody.birthdate).toBe('2020-05-01')

    // Form closed — header trigger is back (and now a unique 'Add Person'
    // button since the empty-state's twin disappears with the list non-empty).
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Add Person' })).toBeInTheDocument()
    })
    expect(screen.queryByRole('heading', { name: 'Add Family Member' })).not.toBeInTheDocument()
  })

  it('edits a person: prefilled form → PUT → card reflects the update', async () => {
    const alice = makePerson({ id: 'p1', name: 'Alice', dietary_goals: 'More protein' })
    mockJson('GET', '/api/people', [alice])

    renderWithProviders(<FamilyManager />)

    await waitFor(() => expect(screen.getByText('Alice')).toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: 'Edit Alice' }))

    // Edit heading confirms we entered edit mode.
    expect(screen.getByRole('heading', { name: 'Edit Alice' })).toBeInTheDocument()

    // First textbox inside the (now-mounted) edit form is the Name input,
    // prefilled with 'Alice'.
    const nameInput = screen.getAllByRole('textbox')[0] as HTMLInputElement
    expect(nameInput.value).toBe('Alice')
    fireEvent.change(nameInput, { target: { value: 'Alicia' } })

    const updated = makePerson({ id: 'p1', name: 'Alicia', dietary_goals: 'More protein' })
    mockJson('PUT', '/api/people/p1', updated)
    mockJson('GET', '/api/people', [updated])

    fireEvent.click(screen.getByRole('button', { name: 'Save Changes' }))

    await waitFor(() => expect(screen.getByText('Alicia')).toBeInTheDocument())
    expect(screen.queryByRole('heading', { name: 'Edit Alice' })).not.toBeInTheDocument()

    const putCall = vi.mocked(fetch).mock.calls.find(([, init]) =>
      (init as RequestInit | undefined)?.method === 'PUT'
    )
    expect(putCall).toBeDefined()
    expect(putCall![0]).toBe('/api/people/p1')
    const putBody = JSON.parse((putCall![1] as RequestInit).body as string)
    expect(putBody.name).toBe('Alicia')
  })

  it('deletes a person: confirm Yes → DELETE → empty state returns', async () => {
    mockJson('GET', '/api/people', [makePerson({ id: 'p1', name: 'Alice' })])

    renderWithProviders(<FamilyManager />)

    await waitFor(() => expect(screen.getByText('Alice')).toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: 'Delete Alice' }))

    // Inline confirmation surfaces.
    expect(screen.getByText('Delete?')).toBeInTheDocument()

    mockJson('DELETE', '/api/people/p1', null, { status: 204 })
    mockJson('GET', '/api/people', [])

    fireEvent.click(screen.getByRole('button', { name: 'Yes' }))

    await waitFor(() => expect(screen.getByText('Your family awaits')).toBeInTheDocument())
    expect(screen.queryByText('Alice')).not.toBeInTheDocument()

    const deleteCall = vi.mocked(fetch).mock.calls.find(([, init]) =>
      (init as RequestInit | undefined)?.method === 'DELETE'
    )
    expect(deleteCall).toBeDefined()
    expect(deleteCall![0]).toBe('/api/people/p1')
  })

  it('surfaces an inline error when create fails', async () => {
    mockJson('GET', '/api/people', [])

    const { container } = renderWithProviders(<FamilyManager />)

    await waitFor(() => expect(screen.getByText('Your family awaits')).toBeInTheDocument())

    // Header + empty-state twins — pick the first (header) to open the form.
    fireEvent.click(screen.getAllByRole('button', { name: 'Add Person' })[0])
    fillRequiredFields(container, { name: 'Carol', birthdate: '2020-05-01' })

    mockJson('POST', '/api/people', { message: 'server exploded' }, { status: 500 })

    fireEvent.click(screen.getByRole('button', { name: 'Add Person' }))

    // Error banner renders String(createMutation.error) — ApiError toString
    // is "ApiError: <message>".
    await waitFor(() => expect(screen.getByText(/server exploded/)).toBeInTheDocument())

    // Form stays open so the user can retry.
    expect(screen.getByRole('heading', { name: 'Add Family Member' })).toBeInTheDocument()
  })
})
