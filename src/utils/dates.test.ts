import { describe, expect, it } from 'vitest'
import {
  addDays,
  formatDateDisplay,
  formatDateKey,
  getMonday,
  getWeekDates,
  isToday,
} from './dates'

describe('getMonday', () => {
  it('returns same day for a Monday', () => {
    const monday = new Date(2025, 5, 9) // June 9, 2025 is Monday
    const result = getMonday(monday)
    expect(result.getDay()).toBe(1)
    expect(result.getDate()).toBe(9)
  })

  it('returns previous Monday for a Wednesday', () => {
    const wed = new Date(2025, 5, 11) // June 11, 2025 is Wednesday
    const result = getMonday(wed)
    expect(result.getDay()).toBe(1)
    expect(result.getDate()).toBe(9)
  })

  it('returns previous Monday for a Sunday', () => {
    const sun = new Date(2025, 5, 15) // June 15, 2025 is Sunday
    const result = getMonday(sun)
    expect(result.getDay()).toBe(1)
    expect(result.getDate()).toBe(9)
  })

  it('handles year boundary', () => {
    const jan1 = new Date(2025, 0, 1) // Wed Jan 1 2025
    const result = getMonday(jan1)
    expect(result.getDay()).toBe(1)
    expect(result.getFullYear()).toBe(2024)
    expect(result.getMonth()).toBe(11) // December
    expect(result.getDate()).toBe(30)
  })

  it('zeroes out time', () => {
    const date = new Date(2025, 5, 9, 14, 30, 45)
    const result = getMonday(date)
    expect(result.getHours()).toBe(0)
    expect(result.getMinutes()).toBe(0)
    expect(result.getSeconds()).toBe(0)
  })
})

describe('formatDateKey', () => {
  it('formats as YYYY-MM-DD', () => {
    expect(formatDateKey(new Date(2025, 0, 5))).toBe('2025-01-05')
    expect(formatDateKey(new Date(2025, 11, 25))).toBe('2025-12-25')
  })

  it('zero-pads month and day', () => {
    expect(formatDateKey(new Date(2025, 0, 1))).toBe('2025-01-01')
    expect(formatDateKey(new Date(2025, 8, 9))).toBe('2025-09-09')
  })
})

describe('formatDateDisplay', () => {
  it('formats as Day M/D', () => {
    const mon = new Date(2025, 5, 9) // Monday June 9
    expect(formatDateDisplay(mon)).toBe('Mon 6/9')
  })

  it('handles Sunday', () => {
    const sun = new Date(2025, 5, 15) // Sunday June 15
    expect(formatDateDisplay(sun)).toBe('Sun 6/15')
  })
})

describe('isToday', () => {
  it('returns true for today', () => {
    expect(isToday(new Date())).toBe(true)
  })

  it('returns false for yesterday', () => {
    const yesterday = addDays(new Date(), -1)
    expect(isToday(yesterday)).toBe(false)
  })
})

describe('getWeekDates', () => {
  it('returns 7 consecutive days starting from input', () => {
    const monday = new Date(2025, 5, 9)
    const dates = getWeekDates(monday)
    expect(dates).toHaveLength(7)
    expect(dates[0].getDate()).toBe(9) // Mon
    expect(dates[6].getDate()).toBe(15) // Sun
  })

  it('each day increments by 1', () => {
    const monday = new Date(2025, 5, 9)
    const dates = getWeekDates(monday)
    for (let i = 1; i < dates.length; i++) {
      const diff = dates[i].getTime() - dates[i - 1].getTime()
      expect(diff).toBe(24 * 60 * 60 * 1000)
    }
  })
})

describe('addDays', () => {
  it('adds positive days', () => {
    const date = new Date(2025, 5, 9)
    const result = addDays(date, 3)
    expect(result.getDate()).toBe(12)
  })

  it('subtracts negative days', () => {
    const date = new Date(2025, 5, 9)
    const result = addDays(date, -7)
    expect(result.getDate()).toBe(2)
  })

  it('handles month boundary', () => {
    const date = new Date(2025, 5, 28) // June 28
    const result = addDays(date, 5) // July 3
    expect(result.getMonth()).toBe(6) // July
    expect(result.getDate()).toBe(3)
  })

  it('does not mutate original date', () => {
    const date = new Date(2025, 5, 9)
    const original = date.getTime()
    addDays(date, 10)
    expect(date.getTime()).toBe(original)
  })
})
