import type { Layout } from '../api/types'

export type Vec3 = [number, number, number]

export interface Placed {
  nodes: Vec3[]
  unions: Vec3[]
  /** Bounding radius around the origin, for framing the camera. */
  radius: number
}

/** Deterministic pseudo-random in [-1, 1) from a string, so the same tree always grows the same way. */
export function jitter(key: string, salt = 0): number {
  let h = 2166136261 ^ salt
  for (let i = 0; i < key.length; i++) h = Math.imul(h ^ key.charCodeAt(i), 16777619)
  return ((h >>> 0) % 20001) / 10000 - 1
}

/**
 * Lift the flat tree layout into 3D: the root person stands at the origin, older generations grow upward,
 * younger ones downward (when `up` is false the direction flips), and each person is nudged in depth so that
 * branches fan out like a real crown instead of lying in one plane.
 */
export function place3d(
  layout: Layout,
  rootIndex: number,
  opts: { unit?: number; depth?: number; up?: boolean } = {},
): Placed {
  const unit = opts.unit ?? 1 / 40
  const depth = opts.depth ?? 6
  const sign = opts.up === false ? -1 : 1
  const root = layout.nodes[rootIndex] ?? layout.nodes[0]
  const ox = root ? root.x + root.w / 2 : 0
  const oy = root ? root.y : 0
  let radius = 1
  const to = (x: number, y: number, key: string): Vec3 => {
    // layout y grows toward younger generations (top-down); invert so ancestors rise
    const p: Vec3 = [(x - ox) * unit, -(y - oy) * unit * sign, jitter(key) * depth]
    radius = Math.max(radius, Math.hypot(p[0], p[1], p[2]))
    return p
  }
  const nodes = layout.nodes.map((n, i) => to(n.x + n.w / 2, n.y, `${n.person_id}:${i}`))
  const unions = layout.unions.map((u) => to(u.x, u.y, u.family_id))
  return { nodes, unions, radius }
}

export interface Theme {
  id: 'garden' | 'night' | 'blueprint'
  background: string
  fog: string
  branch: string
  male: string
  female: string
  other: string
  union: string
  label: string
  labelBg: string
}

export const THEMES: Record<Theme['id'], Theme> = {
  garden: {
    id: 'garden',
    background: '#cfe8d2',
    fog: '#cfe8d2',
    branch: '#6b4a2b',
    male: '#3f7f3a',
    female: '#c2557f',
    other: '#8a8f3c',
    union: '#8a5a2b',
    label: '#14301a',
    labelBg: 'rgba(255,255,255,0.88)',
  },
  night: {
    id: 'night',
    background: '#0a0f1f',
    fog: '#0a0f1f',
    branch: '#7388b3',
    male: '#5cc8ff',
    female: '#ff8fc1',
    other: '#ffd166',
    union: '#b7c3e6',
    label: '#eaf1ff',
    labelBg: 'rgba(10,15,31,0.82)',
  },
  blueprint: {
    id: 'blueprint',
    background: '#0f3a6b',
    fog: '#0f3a6b',
    branch: '#a9d1ff',
    male: '#ffffff',
    female: '#ffe28a',
    other: '#bfe3ff',
    union: '#7fb6f2',
    label: '#ffffff',
    labelBg: 'rgba(15,58,107,0.85)',
  },
}
