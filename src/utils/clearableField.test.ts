import { describe, expect, it } from 'vitest'
import { clearableField } from './clearableField'

describe('clearableField', () => {
  it('sends a set value as is', () => {
    expect(clearableField('salt', null)).toBe('salt')
    expect(clearableField({ value: 5 }, 'x')).toEqual({ value: 5 })
  })

  it('sends null for a field that had a value at load and is empty now', () => {
    expect(clearableField('', 'Use Campari')).toBeNull()
    expect(clearableField(undefined, '{"value":10,"unit":"minutes"}')).toBeNull()
  })

  it('does not treat 0 as empty on either side', () => {
    expect(clearableField(0, null)).toBe(0)
    expect(clearableField(undefined, 0)).toBeNull()
  })

  it('leaves out a field that was empty at load and is empty now', () => {
    expect(clearableField('', null)).toBeUndefined()
    expect(clearableField(undefined, undefined)).toBeUndefined()
    expect(clearableField('', '')).toBeUndefined()
  })
})
