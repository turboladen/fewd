import { describe, expect, it } from 'vitest'
import type { Person } from './person'
import { parsePerson } from './person'

describe('parsePerson', () => {
  const makePerson = (overrides: Partial<Person> = {}): Person => ({
    id: 'test-id',
    name: 'Alice',
    birthdate: '2000-01-15',
    dietary_goals: null,
    dislikes: JSON.stringify(['olives', 'mushrooms']),
    favorites: JSON.stringify(['pasta', 'pizza']),
    notes: null,
    is_active: true,
    created_at: '2025-01-01T00:00:00Z',
    updated_at: '2025-01-01T00:00:00Z',
    ...overrides,
  })

  it('parses dislikes and favorites from JSON', () => {
    const parsed = parsePerson(makePerson())
    expect(parsed.dislikes).toEqual(['olives', 'mushrooms'])
    expect(parsed.favorites).toEqual(['pasta', 'pizza'])
  })

  it('handles empty arrays', () => {
    const parsed = parsePerson(makePerson({
      dislikes: JSON.stringify([]),
      favorites: JSON.stringify([]),
    }))
    expect(parsed.dislikes).toEqual([])
    expect(parsed.favorites).toEqual([])
  })

  it('preserves other fields', () => {
    const parsed = parsePerson(makePerson({ name: 'Bob', dietary_goals: 'low carb' }))
    expect(parsed.name).toBe('Bob')
    expect(parsed.dietary_goals).toBe('low carb')
    expect(parsed.is_active).toBe(true)
  })
})
