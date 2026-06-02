import { describe, expect, it } from 'vitest'
import type { Ingredient, IngredientAmount, Recipe, TimeValue } from './recipe'
import {
  formatAmount,
  formatIngredientLabel,
  formatRatio,
  formatTime,
  ingredientRatio,
  parseInstructionSections,
  parseInstructionSteps,
  parseRecipe,
} from './recipe'

describe('formatTime', () => {
  it('formats time value', () => {
    expect(formatTime({ value: 15, unit: 'minutes' })).toBe('15 minutes')
    expect(formatTime({ value: 2, unit: 'hours' })).toBe('2 hours')
  })

  it('returns empty string for null', () => {
    expect(formatTime(null)).toBe('')
  })
})

describe('formatIngredientLabel', () => {
  const base = (overrides: Partial<Ingredient>): Ingredient => ({
    name: 'garlic',
    amount: { type: 'single', value: 1 },
    unit: 'clove',
    ...overrides,
  })

  it('returns name alone when prep is missing', () => {
    expect(formatIngredientLabel(base({}))).toBe('garlic')
  })

  it('appends prep with comma when present', () => {
    expect(formatIngredientLabel(base({ prep: 'minced' }))).toBe('garlic, minced')
  })

  it('treats empty-string prep as absent', () => {
    expect(formatIngredientLabel(base({ prep: '' }))).toBe('garlic')
  })

  it('treats whitespace-only prep as absent', () => {
    expect(formatIngredientLabel(base({ prep: '   ' }))).toBe('garlic')
    expect(formatIngredientLabel(base({ prep: '\t\n' }))).toBe('garlic')
  })

  it('trims surrounding whitespace before rendering', () => {
    expect(formatIngredientLabel(base({ prep: '  minced  ' }))).toBe('garlic, minced')
  })
})

describe('formatAmount', () => {
  it('formats integer single amount', () => {
    const amount: IngredientAmount = { type: 'single', value: 3 }
    expect(formatAmount(amount)).toBe('3')
  })

  it('formats decimal single amount without trailing zeros', () => {
    const amount: IngredientAmount = { type: 'single', value: 1.5 }
    expect(formatAmount(amount)).toBe('1.5')
  })

  it('formats very precise decimal', () => {
    const amount: IngredientAmount = { type: 'single', value: 0.25 }
    expect(formatAmount(amount)).toBe('0.25')
  })

  it('formats range amount', () => {
    const amount: IngredientAmount = { type: 'range', min: 1, max: 2 }
    expect(formatAmount(amount)).toBe('1-2')
  })
})

describe('parseRecipe', () => {
  const makeRecipe = (overrides: Partial<Recipe> = {}): Recipe => ({
    id: 'test-id',
    slug: 'test-recipe',
    name: 'Test Recipe',
    description: null,
    source: 'manual',
    parent_recipe_id: null,
    prep_time: null,
    cook_time: null,
    total_time: null,
    servings: 4,
    portion_size: null,
    instructions: 'Mix and cook',
    ingredients: JSON.stringify([
      { name: 'flour', amount: { type: 'single', value: 2 }, unit: 'cups' },
    ]),
    nutrition_per_serving: null,
    tags: JSON.stringify(['dinner']),
    notes: null,
    icon: null,
    is_favorite: false,
    times_planned: 0,
    last_planned: null,
    rating: null,
    created_at: '2025-01-01T00:00:00Z',
    updated_at: '2025-01-01T00:00:00Z',
    ...overrides,
  })

  it('parses JSON string fields', () => {
    const parsed = parseRecipe(makeRecipe())
    expect(parsed.ingredients).toHaveLength(1)
    expect(parsed.ingredients[0].name).toBe('flour')
    expect(parsed.tags).toEqual(['dinner'])
  })

  it('parses time values from JSON', () => {
    const time: TimeValue = { value: 15, unit: 'minutes' }
    const parsed = parseRecipe(makeRecipe({
      prep_time: JSON.stringify(time),
    }))
    expect(parsed.prep_time).toEqual(time)
  })

  it('handles null optional fields', () => {
    const parsed = parseRecipe(makeRecipe())
    expect(parsed.prep_time).toBeNull()
    expect(parsed.cook_time).toBeNull()
    expect(parsed.nutrition_per_serving).toBeNull()
    expect(parsed.portion_size).toBeNull()
  })

  it('preserves non-JSON fields', () => {
    const parsed = parseRecipe(makeRecipe({ name: 'Pasta', servings: 6 }))
    expect(parsed.name).toBe('Pasta')
    expect(parsed.servings).toBe(6)
    expect(parsed.is_favorite).toBe(false)
  })

  it('preserves rating value', () => {
    const parsed = parseRecipe(makeRecipe({ rating: 4 }))
    expect(parsed.rating).toBe(4)
  })

  it('handles null rating', () => {
    const parsed = parseRecipe(makeRecipe({ rating: null }))
    expect(parsed.rating).toBeNull()
  })

  it('preserves parent_recipe_id', () => {
    const parsed = parseRecipe(makeRecipe({ parent_recipe_id: 'parent-123' }))
    expect(parsed.parent_recipe_id).toBe('parent-123')
  })

  it('handles null parent_recipe_id', () => {
    const parsed = parseRecipe(makeRecipe({ parent_recipe_id: null }))
    expect(parsed.parent_recipe_id).toBeNull()
  })
})

describe('parseInstructionSteps', () => {
  it('splits a multi-line block into one step per line', () => {
    const steps = parseInstructionSteps('Boil water.\nAdd pasta.\nStir occasionally.')
    expect(steps).toEqual(['Boil water.', 'Add pasta.', 'Stir occasionally.'])
  })

  it('strips leading numbers like "1." or "2)" so they can be re-rendered', () => {
    const steps = parseInstructionSteps('1. Boil water.\n2. Add pasta.\n3) Stir.')
    expect(steps).toEqual(['Boil water.', 'Add pasta.', 'Stir.'])
  })

  it('treats blank lines as separators, not as steps', () => {
    const steps = parseInstructionSteps('Boil water.\n\nAdd pasta.\n\n\nStir.')
    expect(steps).toEqual(['Boil water.', 'Add pasta.', 'Stir.'])
  })

  it('returns a single step when the input has no line breaks', () => {
    const steps = parseInstructionSteps('Just do the thing.')
    expect(steps).toEqual(['Just do the thing.'])
  })

  it('returns an empty array for empty or whitespace-only input', () => {
    expect(parseInstructionSteps('')).toEqual([])
    expect(parseInstructionSteps('   \n  \n')).toEqual([])
  })

  it('trims surrounding whitespace from each step', () => {
    const steps = parseInstructionSteps('  Boil water.  \n  Add pasta.  ')
    expect(steps).toEqual(['Boil water.', 'Add pasta.'])
  })

  it('joins soft-wrapped lines within a paragraph into a single step', () => {
    const enhanced = [
      'Heat 4 tbsp olive oil in a large heavy pot or Dutch oven over medium heat. Add onion,',
      'carrot, and celery. Cook, stirring occasionally, until very soft and starting',
      "to turn golden — about 15 minutes. Don't rush this.",
      '',
      'Add garlic and cook 1 minute more.',
      '',
      'Increase heat to medium-high. Add ground beef and pork in chunks. Break up',
      'and brown thoroughly. Season generously with salt and pepper.',
    ].join('\n')
    const steps = parseInstructionSteps(enhanced)

    expect(steps).toHaveLength(3)
    expect(steps[0]).toContain('Heat 4 tbsp olive oil')
    expect(steps[0]).toContain('until very soft')
    expect(steps[0]).toContain("Don't rush this.")
    expect(steps[0]).not.toContain('\n')
    expect(steps[1]).toBe('Add garlic and cook 1 minute more.')
    expect(steps[2]).toContain('Increase heat to medium-high.')
    expect(steps[2]).toContain('Season generously with salt and pepper.')
  })

  it('keeps a numbered item with continuation lines as one step', () => {
    const steps = parseInstructionSteps('1. Heat oil.\n   Add onions.\n2. Add garlic.')
    expect(steps).toEqual(['Heat oil. Add onions.', 'Add garlic.'])
  })

  it('handles numbered lists where each item spans paragraph breaks', () => {
    const input = '1. Heat oil and cook\nthe vegetables.\n\n2. Add garlic.\n\n3. Brown the meat.'
    const steps = parseInstructionSteps(input)
    expect(steps).toEqual([
      'Heat oil and cook the vegetables.',
      'Add garlic.',
      'Brown the meat.',
    ])
  })

  it('joins soft-wraps in unnumbered prose paragraphs', () => {
    const input =
      'Heat oil in a large pan\nover medium heat.\n\nAdd onions and cook\nuntil translucent.'
    const steps = parseInstructionSteps(input)
    expect(steps).toEqual([
      'Heat oil in a large pan over medium heat.',
      'Add onions and cook until translucent.',
    ])
  })

  it('splits a contiguous numbered list even when a blank line follows it', () => {
    // Regression: a numbered list (no blanks between items) followed by a
    // blank-separated trailing block must not collapse into one merged step.
    const input = '1. Whisk the eggs.\n2. Stir in milk.\n\n- Garnish with mint'
    const steps = parseInstructionSteps(input)
    expect(steps[0]).toBe('Whisk the eggs.')
    expect(steps[1]).toBe('Stir in milk.')
    expect(steps.some((s) => s.includes('2.'))).toBe(false)
  })

  it('does not mistake a decimal quantity for a leading list marker', () => {
    // stripMarker requires whitespace after the marker, so "1.5" stays intact
    // instead of being read as a "1." list marker (which would yield "5 cups").
    const steps = parseInstructionSteps('Mix dry goods.\n\n1.5 cups flour, then stir')
    expect(steps).toEqual(['Mix dry goods.', '1.5 cups flour, then stir'])
  })

  it('strips leading bullet markers so cook-mode cards do not double-mark', () => {
    const steps = parseInstructionSteps('- Garnish with mint\n- Serve immediately')
    expect(steps).toEqual(['Garnish with mint', 'Serve immediately'])
  })

  it('normalizes CRLF line endings before splitting', () => {
    const input = 'Heat oil in a pan\r\nover medium heat.\r\n\r\nAdd onions.'
    const steps = parseInstructionSteps(input)
    expect(steps).toEqual([
      'Heat oil in a pan over medium heat.',
      'Add onions.',
    ])
  })
})

describe('parseInstructionSections', () => {
  it('returns a single null-heading section when there are no headings', () => {
    const sections = parseInstructionSections('1. Boil water.\n2. Add pasta.')
    expect(sections).toEqual([
      { heading: null, steps: ['Boil water.', 'Add pasta.'] },
    ])
  })

  it('groups steps under each markdown heading and strips the # markers', () => {
    const input = [
      '## Caramelized Pineapple',
      '1. Melt butter.',
      '2. Add pineapple.',
      '',
      '## Custard Base',
      '1. Whisk eggs.',
      '2. Temper with cream.',
    ].join('\n')
    expect(parseInstructionSections(input)).toEqual([
      { heading: 'Caramelized Pineapple', steps: ['Melt butter.', 'Add pineapple.'] },
      { heading: 'Custard Base', steps: ['Whisk eggs.', 'Temper with cream.'] },
    ])
  })

  it('keeps pre-heading steps as a leading null-heading section', () => {
    const input = 'Preheat the oven.\n\n## Filling\n1. Mix.\n2. Pour.'
    expect(parseInstructionSections(input)).toEqual([
      { heading: null, steps: ['Preheat the oven.'] },
      { heading: 'Filling', steps: ['Mix.', 'Pour.'] },
    ])
  })

  it('never leaks a literal ## marker into a step', () => {
    const input = '## Section A\n1. Do a thing.\n### Sub\n- bullet item'
    const sections = parseInstructionSections(input)
    const allSteps = sections.flatMap((s) => s.steps)
    expect(allSteps.some((step) => step.includes('#'))).toBe(false)
    expect(sections.map((s) => s.heading)).toEqual(['Section A', 'Sub'])
  })

  it('drops a heading that has no steps instead of emitting an empty section', () => {
    const sections = parseInstructionSections('## Empty Section\n## Real\n1. Step')
    expect(sections).toEqual([{ heading: 'Real', steps: ['Step'] }])
  })

  it('strips the ATX closing # sequence from heading text', () => {
    const sections = parseInstructionSections('## Custard Base ##\n1. Whisk.')
    expect(sections).toEqual([{ heading: 'Custard Base', steps: ['Whisk.'] }])
  })

  it('returns an empty array for empty input', () => {
    expect(parseInstructionSections('')).toEqual([])
    expect(parseInstructionSections('   \n  \n')).toEqual([])
  })
})

describe('ingredientRatio', () => {
  it('returns current.value / original.value for single amounts', () => {
    const original: IngredientAmount = { type: 'single', value: 3 }
    const current: IngredientAmount = { type: 'single', value: 5 }
    expect(ingredientRatio(current, original)).toBeCloseTo(1.667, 3)
  })

  it('uses min on either side when the amount is a range', () => {
    const original: IngredientAmount = { type: 'range', min: 2, max: 4 }
    const current: IngredientAmount = { type: 'single', value: 3 }
    expect(ingredientRatio(current, original)).toBe(1.5)
  })

  it('uses min on both sides when neither has collapsed to single yet', () => {
    const original: IngredientAmount = { type: 'range', min: 2, max: 4 }
    const current: IngredientAmount = { type: 'range', min: 3, max: 5 }
    expect(ingredientRatio(current, original)).toBe(1.5)
  })

  it('returns null when the original value is zero', () => {
    const original: IngredientAmount = { type: 'single', value: 0 }
    const current: IngredientAmount = { type: 'single', value: 1 }
    expect(ingredientRatio(current, original)).toBeNull()
  })

  it('returns 1 when current matches original exactly', () => {
    const original: IngredientAmount = { type: 'single', value: 2 }
    const current: IngredientAmount = { type: 'single', value: 2 }
    expect(ingredientRatio(current, original)).toBe(1)
  })
})

describe('formatRatio', () => {
  it('renders 1.0 as "1×"', () => {
    expect(formatRatio(1.0)).toBe('1×')
  })

  it('renders 1.667 as "1.67×"', () => {
    expect(formatRatio(1.667)).toBe('1.67×')
  })

  it('trims trailing zeros after the decimal', () => {
    expect(formatRatio(1.5)).toBe('1.5×')
    expect(formatRatio(1.5000001)).toBe('1.5×')
  })

  it('handles sub-1 ratios', () => {
    expect(formatRatio(0.5)).toBe('0.5×')
  })

  it('renders null as em dash', () => {
    expect(formatRatio(null)).toBe('—')
  })
})
