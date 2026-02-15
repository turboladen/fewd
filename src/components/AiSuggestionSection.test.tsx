import { fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

// Mock hooks
const mockUseSetting = vi.fn()
const mockAiMutate = vi.fn()
const mockCreateMutate = vi.fn()

vi.mock('../hooks/useSettings', () => ({
  useSetting: () => mockUseSetting(),
}))

vi.mock('../hooks/useSuggestions', () => ({
  useAiSuggestMeals: () => ({
    mutate: mockAiMutate,
    isPending: false,
    error: null,
    data: null,
    progress: null,
    reset: vi.fn(),
  }),
}))

vi.mock('../hooks/useRecipes', () => ({
  useCreateRecipe: () => ({
    mutate: mockCreateMutate,
    isPending: false,
    error: null,
  }),
}))

import { AiSuggestionSection } from './AiSuggestionSection'

const mockPeople = [
  {
    id: 'p1',
    name: 'Steve',
    birthdate: '1990-01-01',
    dietary_goals: '2400 cal, 190g protein',
    dislikes: '["mushrooms"]',
    favorites: '["ramen"]',
    notes: null,
    is_active: true,
    created_at: '2025-01-01',
    updated_at: '2025-01-01',
  },
  {
    id: 'p2',
    name: 'Amanda',
    birthdate: '1992-05-15',
    dietary_goals: null,
    dislikes: '["olives"]',
    favorites: '["pasta"]',
    notes: null,
    is_active: true,
    created_at: '2025-01-01',
    updated_at: '2025-01-01',
  },
]

const defaultProps = {
  people: mockPeople,
  selectedPersonIds: new Set(['p1', 'p2']),
  mealType: 'Dinner',
  onApply: vi.fn(),
}

describe('AiSuggestionSection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockUseSetting.mockReturnValue({ data: 'sk-ant-test-key', isLoading: false })
  })

  it('shows no-API-key message when key is not set', () => {
    mockUseSetting.mockReturnValue({ data: null, isLoading: false })
    render(<AiSuggestionSection {...defaultProps} />)
    expect(screen.getByText(/Set your Anthropic API key/)).toBeInTheDocument()
  })

  it('shows meal character radio buttons', () => {
    render(<AiSuggestionSection {...defaultProps} />)
    expect(screen.getByText('Balanced')).toBeInTheDocument()
    expect(screen.getByText('Indulgent')).toBeInTheDocument()
    expect(screen.getByText('Quick & Easy')).toBeInTheDocument()
    expect(screen.getByText('Custom')).toBeInTheDocument()
  })

  it('shows field toggles for selected people', () => {
    render(<AiSuggestionSection {...defaultProps} />)
    expect(screen.getByText('Steve')).toBeInTheDocument()
    expect(screen.getByText('Amanda')).toBeInTheDocument()
    const goalButtons = screen.getAllByText('Dietary goals')
    expect(goalButtons.length).toBe(2)
  })

  it('does not show field toggles for unselected people', () => {
    render(
      <AiSuggestionSection
        {...defaultProps}
        selectedPersonIds={new Set(['p1'])}
      />,
    )
    expect(screen.getByText('Steve')).toBeInTheDocument()
    expect(screen.queryByText('Amanda')).not.toBeInTheDocument()
  })

  it('disables generate when no people selected', () => {
    render(
      <AiSuggestionSection
        {...defaultProps}
        selectedPersonIds={new Set()}
      />,
    )
    const btn = screen.getByText('Generate AI Suggestions')
    expect(btn).toBeDisabled()
  })

  it('calls ai_suggest_meals mutation on generate', () => {
    render(<AiSuggestionSection {...defaultProps} />)
    fireEvent.click(screen.getByText('Generate AI Suggestions'))
    expect(mockAiMutate).toHaveBeenCalledOnce()
    expect(mockAiMutate).toHaveBeenCalledWith(
      expect.objectContaining({
        meal_type: 'Dinner',
        character: { type: 'balanced' },
        person_options: expect.arrayContaining([
          expect.objectContaining({ person_id: 'p1' }),
          expect.objectContaining({ person_id: 'p2' }),
        ]),
      }),
      expect.any(Object),
    )
  })

  it('shows custom text input when Custom character selected', () => {
    render(<AiSuggestionSection {...defaultProps} />)
    fireEvent.click(screen.getByText('Custom'))
    expect(screen.getByPlaceholderText(/Mediterranean/)).toBeInTheDocument()
  })
})
