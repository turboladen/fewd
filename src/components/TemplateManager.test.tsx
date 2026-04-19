import { fireEvent, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { makeMealTemplate, makePerson, makeRecipe } from '../test/factories'
import { installFetchMock, mockJson, resetFetchMock } from '../test/fetchMock'
import { renderWithProviders } from '../test/renderWithProviders'
import type { PersonServing } from '../types/meal'
import { TemplateManager } from './TemplateManager'

beforeEach(() => installFetchMock())
afterEach(() => resetFetchMock())

function seedBaseline(templates = [] as ReturnType<typeof makeMealTemplate>[]) {
  mockJson('GET', '/api/meal-templates', templates)
  mockJson('GET', '/api/people', [makePerson({ id: 'p1', name: 'Alice' })])
  mockJson('GET', '/api/recipes', [makeRecipe({ id: 'r-pasta', name: 'Pasta' })])
}

describe('TemplateManager', () => {
  it('groups templates by meal type, showing a header per group', async () => {
    const aliceServing: PersonServing = {
      food_type: 'recipe',
      person_id: 'p1',
      recipe_id: 'r-pasta',
      servings_count: 1,
      notes: null,
    }
    seedBaseline([
      makeMealTemplate({
        id: 't1',
        name: 'Pasta Night',
        meal_type: 'Dinner',
        servings: [aliceServing],
      }),
      makeMealTemplate({
        id: 't2',
        name: 'Quick Oats',
        meal_type: 'Breakfast',
        servings: [aliceServing],
      }),
    ])

    renderWithProviders(<TemplateManager />)

    await screen.findByText('Pasta Night')
    expect(screen.getByText('Quick Oats')).toBeInTheDocument()
    // Group headers render as uppercase h2 labels for each meal type.
    expect(screen.getByRole('heading', { name: 'Breakfast' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Dinner' })).toBeInTheDocument()
  })

  it('deletes a template after Yes-confirmation and invalidates the meal_templates query', async () => {
    const aliceServing: PersonServing = {
      food_type: 'recipe',
      person_id: 'p1',
      recipe_id: 'r-pasta',
      servings_count: 1,
      notes: null,
    }
    seedBaseline([
      makeMealTemplate({
        id: 't-delete',
        name: 'Going Away',
        meal_type: 'Dinner',
        servings: [aliceServing],
      }),
    ])

    const { client } = renderWithProviders(<TemplateManager />)
    const invalidateSpy = vi.spyOn(client, 'invalidateQueries')

    await screen.findByText('Going Away')

    // Stage DELETE + shadowed empty refetch.
    mockJson('DELETE', '/api/meal-templates/t-delete', null, { status: 204 })
    mockJson('GET', '/api/meal-templates', [])

    fireEvent.click(screen.getByRole('button', { name: 'Delete' }))
    fireEvent.click(screen.getByRole('button', { name: 'Yes' }))

    await waitFor(() => expect(screen.queryByText('Going Away')).not.toBeInTheDocument())
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['meal_templates'] })
  })

  it('shows an empty state when no templates exist', async () => {
    seedBaseline([])

    renderWithProviders(<TemplateManager />)

    await screen.findByText('No templates yet')
    expect(screen.getByText(/Save a meal as a template/)).toBeInTheDocument()
  })
})
