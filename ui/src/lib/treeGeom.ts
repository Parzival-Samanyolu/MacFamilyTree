import type { LEdge, LNode, Layout } from '../api/types'

export interface Transform {
  k: number
  tx: number
  ty: number
}

export const K_MIN = 0.08
export const K_MAX = 3

export function fitTransform(contentW: number, contentH: number, viewW: number, viewH: number, pad = 24): Transform {
  if (contentW <= 0 || contentH <= 0 || viewW <= 0 || viewH <= 0) return { k: 1, tx: 0, ty: 0 }
  const k = Math.min(1.25, Math.max(K_MIN, Math.min((viewW - pad * 2) / contentW, (viewH - pad * 2) / contentH)))
  return { k, tx: (viewW - contentW * k) / 2, ty: (viewH - contentH * k) / 2 }
}

/** Zoom keeping the content point under (px, py) fixed. */
export function zoomAt(t: Transform, px: number, py: number, factor: number): Transform {
  const k = Math.min(K_MAX, Math.max(K_MIN, t.k * factor))
  const f = k / t.k
  return { k, tx: px - (px - t.tx) * f, ty: py - (py - t.ty) * f }
}

/** Transform that centres content point (cx, cy) in the view at the current zoom. */
export function centerOn(t: Transform, cx: number, cy: number, viewW: number, viewH: number): Transform {
  return { k: t.k, tx: viewW / 2 - cx * t.k, ty: viewH / 2 - cy * t.k }
}

export function visibleIndices(nodes: LNode[], t: Transform, viewW: number, viewH: number, margin = 80): number[] {
  const x0 = (-t.tx - margin) / t.k
  const y0 = (-t.ty - margin) / t.k
  const x1 = (viewW - t.tx + margin) / t.k
  const y1 = (viewH - t.ty + margin) / t.k
  const out: number[] = []
  nodes.forEach((n, i) => {
    if (n.x + n.w >= x0 && n.x <= x1 && n.y + n.h >= y0 && n.y <= y1) out.push(i)
  })
  return out
}

export type Dir = 'up' | 'down' | 'left' | 'right'

/** Nearest node in a direction (arrow-key navigation). */
export function nearest(nodes: LNode[], from: number, dir: Dir): number | null {
  const a = nodes[from]
  if (!a) return null
  const ax = a.x + a.w / 2
  const ay = a.y + a.h / 2
  let best: number | null = null
  let bestScore = Infinity
  nodes.forEach((n, i) => {
    if (i === from) return
    const dx = n.x + n.w / 2 - ax
    const dy = n.y + n.h / 2 - ay
    const primary = dir === 'up' ? -dy : dir === 'down' ? dy : dir === 'left' ? -dx : dx
    const lateral = dir === 'up' || dir === 'down' ? Math.abs(dx) : Math.abs(dy)
    if (primary <= 1) return
    const score = primary + lateral * 2.5
    if (score < bestScore) {
      bestScore = score
      best = i
    }
  })
  return best
}

type Pt = [number, number]

function round(pts: Pt[]): string {
  return pts.map((p, i) => `${i === 0 ? 'M' : 'L'}${p[0].toFixed(1)} ${p[1].toFixed(1)}`).join(' ')
}

/** Orthogonal connector between two anchor points, bending half way along the main axis. */
export function elbow(a: Pt, b: Pt, vertical: boolean): string {
  if (vertical) {
    const my = (a[1] + b[1]) / 2
    return round([a, [a[0], my], [b[0], my], b])
  }
  const mx = (a[0] + b[0]) / 2
  return round([a, [mx, a[1]], [mx, b[1]], b])
}

export function edgePath(l: Layout, e: LEdge, vertical: boolean): string {
  if (e.kind === 'Child' && !e.from_union) {
    // ancestor direction: from child card to parent card
    const c = l.nodes[e.from]
    const p = l.nodes[e.to]
    if (vertical) {
      const up = p.y < c.y
      return elbow([c.x + c.w / 2, up ? c.y : c.y + c.h], [p.x + p.w / 2, up ? p.y + p.h : p.y], true)
    }
    const left = p.x < c.x
    return elbow([left ? c.x : c.x + c.w, c.y + c.h / 2], [left ? p.x + p.w : p.x, p.y + p.h / 2], false)
  }
  if (e.kind === 'Child') {
    // union -> child
    const u = l.unions[e.from]
    const c = l.nodes[e.to]
    if (vertical) {
      const down = c.y > u.y
      return elbow([u.x, down ? u.y + c.h / 2 : u.y - c.h / 2], [c.x + c.w / 2, down ? c.y : c.y + c.h], true)
    }
    const right = c.x > u.x
    return elbow([right ? u.x + c.w / 2 : u.x - c.w / 2, u.y], [right ? c.x : c.x + c.w, c.y + c.h / 2], false)
  }
  // person -> union (partner line)
  const n = l.nodes[e.from]
  const u = l.unions[e.to]
  return round([
    [n.x + n.w / 2, n.y + n.h / 2],
    [u.x, u.y],
  ])
}

export function boundsOf(l: Layout): { w: number; h: number } {
  return { w: l.width, h: l.height }
}
