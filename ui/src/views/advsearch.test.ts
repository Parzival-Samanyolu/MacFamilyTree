import { describe, expect, it } from 'vitest'
import { activeCount, EMPTY } from './AdvancedSearch'

describe('activeCount', () => {
  it('counts restricting criteria only', () => {
    expect(activeCount(EMPTY)).toBe(0)
    expect(activeCount({ ...EMPTY, phonetic: true })).toBe(0)
    expect(activeCount({ ...EMPTY, surname: 'Kaya', born_from: 1850, has_media: false, sex: '' })).toBe(3)
  })
})
