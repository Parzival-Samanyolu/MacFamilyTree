import { describe, expect, it } from 'vitest'
import type { Layout } from '../api/types'
import { jitter, place3d } from './virtual3d'

const node = (id: string, x: number, y: number, generation: number) => ({
  person_id: id,
  x,
  y,
  w: 100,
  h: 50,
  generation,
  dup_of: null,
  has_more: false,
  is_spouse: false,
})
const layout: Layout = {
  nodes: [node('root', 200, 300, 0), node('dad', 120, 150, -1), node('mum', 280, 150, -1), node('kid', 200, 450, 1)],
  unions: [{ family_id: 'f1', x: 200, y: 150 }],
  edges: [],
  width: 400,
  height: 600,
}

describe('place3d', () => {
  it('puts the root at the origin, ancestors above and descendants below', () => {
    const p = place3d(layout, 0)
    expect(p.nodes[0][0]).toBeCloseTo(0)
    expect(p.nodes[0][1]).toBeCloseTo(0)
    expect(p.nodes[1][1]).toBeGreaterThan(0)
    expect(p.nodes[3][1]).toBeLessThan(0)
    expect(p.nodes[1][0]).toBeLessThan(p.nodes[2][0])
    expect(p.unions[0][1]).toBeCloseTo(p.nodes[1][1])
  })
  it('is deterministic and gives depth variation within bounds', () => {
    const a = place3d(layout, 0, { depth: 4 })
    const b = place3d(layout, 0, { depth: 4 })
    expect(a).toEqual(b)
    for (const n of a.nodes) expect(Math.abs(n[2])).toBeLessThanOrEqual(4)
    expect(new Set(a.nodes.map((n) => n[2].toFixed(3))).size).toBeGreaterThan(1)
    expect(a.radius).toBeGreaterThan(1)
  })
  it('can flip so the tree grows downward', () => {
    expect(place3d(layout, 0, { up: false }).nodes[1][1]).toBeLessThan(0)
  })
  it('jitter stays in range', () => {
    for (const k of ['a', 'bb', 'ccc', 'Ünïcode']) expect(Math.abs(jitter(k))).toBeLessThanOrEqual(1)
  })
})
