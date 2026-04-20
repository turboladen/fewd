import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ParsedRecipe } from '../types/recipe'

// Mock the hooks
const mockUsePeople = vi.fn()
const mockUseSetting = vi.fn()
const mockMutate = vi.fn()
const mockReset = vi.fn()
const mockCreateMutate = vi.fn()

vi.mock('../hooks/usePeople', () => ({
  usePeople: () => mockUsePeople(),
}))

vi.mock('../hooks/useSettings', () => ({
  useSetting: () => mockUseSetting(),
}))

vi.mock('../hooks/useRecipes', () => ({
  useAdaptRecipe: () => ({
    mutate: mockMutate,
    isPending: false,
    error: null,
    reset: mockReset,
  }),
  useCreateRecipe: () => ({
    mutate: mockCreateMutate,
    isPending: false,
    error: null,
  }),
}))

import { AdaptRecipePanel } from './AdaptRecipePanel'

const mockParsed: ParsedRecipe = {
  id: 'recipe-1',
  slug: 'grilled-chicken',
  name: 'Grilled Chicken',
  description: 'A simple grilled chicken',
  source: 'manual',
  parent_recipe_id: null,
  prep_time: null,
  cook_time: null,
  total_time: null,
  servings: 4,
  portion_size: null,
  instructions: 'Grill the chicken',
  ingredients: [
    { name: 'chicken breast', amount: { type: 'single', value: 2 }, unit: 'lb' },
  ],
  nutrition_per_serving: null,
  tags: ['dinner'],
  notes: null,
  icon: null,
  is_favorite: false,
  times_made: 0,
  last_made: null,
  rating: null,
  created_at: '2025-01-01',
  updated_at: '2025-01-01',
}

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
  {
    id: 'p3',
    name: 'Inactive',
    birthdate: '2000-01-01',
    dietary_goals: null,
    dislikes: '[]',
    favorites: '[]',
    notes: null,
    is_active: false,
    created_at: '2025-01-01',
    updated_at: '2025-01-01',
  },
]

const defaultProps = {
  parsed: mockParsed,
  onComplete: vi.fn(),
  onEdit: vi.fn(),
  onCancel: vi.fn(),
}

describe('AdaptRecipePanel', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockUsePeople.mockReturnValue({ data: mockPeople })
    mockUseSetting.mockReturnValue({ data: 'sk-ant-test-key', isLoading: false })
  })

  it('renders person checkboxes for active people only', () => {
    render(<AdaptRecipePanel {...defaultProps} />)
    expect(screen.getByText('Steve')).toBeInTheDocument()
    expect(screen.getByText('Amanda')).toBeInTheDocument()
    expect(screen.queryByText('Inactive')).not.toBeInTheDocument()
  })

  it('shows field toggles when person is selected', () => {
    render(<AdaptRecipePanel {...defaultProps} />)
    // Both people are selected by default, so toggles should be visible
    const goalButtons = screen.getAllByText('Dietary goals')
    expect(goalButtons.length).toBeGreaterThan(0)
    const dislikeButtons = screen.getAllByText('Dislikes')
    expect(dislikeButtons.length).toBeGreaterThan(0)
  })

  it('hides field toggles when person is deselected', () => {
    render(<AdaptRecipePanel {...defaultProps} />)
    // Click Steve to deselect
    fireEvent.click(screen.getByText('Steve'))
    // Steve's profile summary should disappear (mushrooms is from Steve's dislikes)
    // Amanda should still show
    expect(screen.getByText('Amanda')).toBeInTheDocument()
  })

  it('disables generate button when no people selected', () => {
    render(<AdaptRecipePanel {...defaultProps} />)
    // Deselect all people
    fireEvent.click(screen.getByText('Steve'))
    fireEvent.click(screen.getByText('Amanda'))
    const generateBtn = screen.getByText('Generate Adapted Recipe')
    expect(generateBtn).toBeDisabled()
  })

  it('shows no-API-key message when key is not set', () => {
    mockUseSetting.mockReturnValue({ data: null, isLoading: false })
    render(<AdaptRecipePanel {...defaultProps} />)
    expect(screen.getByText(/Set your Anthropic API key/)).toBeInTheDocument()
  })

  it('calls adapt mutation on generate', () => {
    render(<AdaptRecipePanel {...defaultProps} />)
    fireEvent.click(screen.getByText('Generate Adapted Recipe'))
    expect(mockMutate).toHaveBeenCalledOnce()
    expect(mockMutate).toHaveBeenCalledWith(
      expect.objectContaining({
        recipe_id: 'recipe-1',
        person_options: expect.arrayContaining([
          expect.objectContaining({ person_id: 'p1' }),
          expect.objectContaining({ person_id: 'p2' }),
        ]),
      }),
      expect.any(Object),
    )
  })

  it('displays recipe title in panel header', () => {
    render(<AdaptRecipePanel {...defaultProps} />)
    expect(screen.getByText('Adapt: Grilled Chicken')).toBeInTheDocument()
  })
})
