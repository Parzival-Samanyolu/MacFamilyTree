import { describe, expect, it } from 'vitest'
import type { LNode, Layout } from '../api/types'
import { centerOn, edgePath, fitTransform, K_MAX, K_MIN, nearest, visibleIndices, zoomAt } from './treeGeom'

const node = (x: number, y: number, extra: Partial<LNode> = {}): LNode => ({
  person_id: `${x},${y}`,
  x,
  y,
  w: 100,
  h: 40,
  generation: 0,
  dup_of: null,
  has_more: false,
  is_spouse: false,
  ...extra,
})

describe('transform maths', () => {
  it('fits content centred and never zooms beyond 1.25x', () => {
    const t = fitTransform(200, 100, 1000, 600)
    expect(t.k).toBeCloseTo(1.25)
    expect(t.tx).toBeCloseTo((1000 - 200 * 1.25) / 2)
    const big = fitTransform(4000, 2000, 1000, 600)
    expect(big.k).toBeLessThan(0.3)
    expect(fitTransform(0, 0, 1000, 600)).toEqual({ k: 1, tx: 0, ty: 0 })
  })
  it('zooms around the pointer so the point under it stays put', () => {
    const t0 = { k: 1, tx: 10, ty: 20 }
    const px = 300
    const py = 200
    const before = [(px - t0.tx) / t0.k, (py - t0.ty) / t0.k]
    const t1 = zoomAt(t0, px, py, 2)
    expect(t1.k).toBe(2)
    expect([(px - t1.tx) / t1.k, (py - t1.ty) / t1.k]).toEqual(before)
  })
  it('clamps zoom', () => {
    expect(zoomAt({ k: 1, tx: 0, ty: 0 }, 0, 0, 1000).k).toBe(K_MAX)
    expect(zoomAt({ k: 1, tx: 0, ty: 0 }, 0, 0, 0.0001).k).toBe(K_MIN)
  })
  it('centres a content point', () => {
    const t = centerOn({ k: 2, tx: 0, ty: 0 }, 50, 30, 400, 200)
    expect(50 * t.k + t.tx).toBe(200)
    expect(30 * t.k + t.ty).toBe(100)
  })
})

describe('culling and navigation', () => {
  const nodes = [node(0, 0), node(1000, 0), node(0, 1000), node(500, 500)]
  it('returns only on-screen nodes', () => {
    expect(visibleIndices(nodes, { k: 1, tx: 0, ty: 0 }, 300, 200, 0)).toEqual([0])
    expect(visibleIndices(nodes, { k: 0.1, tx: 0, ty: 0 }, 1200, 1200, 0)).toEqual([0, 1, 2, 3])
  })
  it('moves the selection to the nearest node in the arrow direction', () => {
    const grid = [node(100, 100), node(300, 100), node(100, 300), node(300, 300), node(900, 130)]
    expect(nearest(grid, 0, 'right')).toBe(1)
    expect(nearest(grid, 0, 'down')).toBe(2)
    expect(nearest(grid, 3, 'up')).toBe(1)
    expect(nearest(grid, 1, 'left')).toBe(0)
    expect(nearest(grid, 0, 'up')).toBeNull()
    expect(nearest(grid, 1, 'right')).toBe(4)
  })
})

describe('edges', () => {
  const layout: Layout = {
    nodes: [node(100, 200), node(20, 80), node(180, 80)],
    unions: [{ family_id: 'f', x: 150, y: 220 }],
    edges: [],
    width: 400,
    height: 400,
  }
  it('draws ancestor edges from the child top to the parent bottom', () => {
    const d = edgePath(layout, { kind: 'Child', from: 0, from_union: false, to: 1, to_union: false, link: '' }, true)
    expect(d.startsWith('M150.0 200.0')).toBe(true)
    expect(d.endsWith('L70.0 120.0')).toBe(true)
    expect(d.match(/L/g)).toHaveLength(3)
  })
  it('draws horizontal elbows for left-right charts', () => {
    const l: Layout = { ...layout, nodes: [node(0, 100), node(200, 40)] }
    const d = edgePath(l, { kind: 'Child', from: 0, from_union: false, to: 1, to_union: false, link: '' }, false)
    expect(d.startsWith('M100.0 120.0')).toBe(true)
    expect(d.endsWith('L200.0 60.0')).toBe(true)
  })
})
