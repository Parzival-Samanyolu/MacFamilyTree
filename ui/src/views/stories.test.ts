import { describe, expect, it } from 'vitest'
import { emptyBlock, moveBlock } from './Stories'

describe('story helpers', () => {
  it('moves blocks within bounds and ignores invalid moves', () => {
    expect(moveBlock([1, 2, 3], 0, 2)).toEqual([2, 3, 1])
    expect(moveBlock([1, 2, 3], 2, 1)).toEqual([1, 3, 2])
    const same = [1, 2]
    expect(moveBlock(same, 0, -1)).toBe(same)
    expect(moveBlock(same, 1, 5)).toBe(same)
  })
  it('creates empty blocks of each type', () => {
    expect(emptyBlock('text')).toEqual({ type: 'text', text: '' })
    expect(emptyBlock('person')).toEqual({ type: 'person', id: '' })
    expect(emptyBlock('quote')).toMatchObject({ type: 'quote' })
  })
})
