import { useQuery } from '@tanstack/react-query'
import { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { Summary } from '../api/types'
import { downloadBlob, sexColor } from '../lib/format'
import { q } from '../lib/query'
import { useApp } from '../store/app'
import { serializeSvg } from './TreeView'

interface Item {
  n: number
  person: Summary
}

const rad = (deg: number) => (deg * Math.PI) / 180
// angle measured clockwise from "up"
const pt = (cx: number, cy: number, r: number, deg: number): [number, number] => [
  cx + r * Math.sin(rad(deg)),
  cy - r * Math.cos(rad(deg)),
]

export function sectorPath(cx: number, cy: number, r0: number, r1: number, a0: number, a1: number): string {
  const large = a1 - a0 > 180 ? 1 : 0
  const [x0, y0] = pt(cx, cy, r1, a0)
  const [x1, y1] = pt(cx, cy, r1, a1)
  const [x2, y2] = pt(cx, cy, r0, a1)
  const [x3, y3] = pt(cx, cy, r0, a0)
  if (r0 <= 0)
    return `M${cx} ${cy} L${x0.toFixed(2)} ${y0.toFixed(2)} A${r1} ${r1} 0 ${large} 1 ${x1.toFixed(2)} ${y1.toFixed(2)} Z`
  return `M${x0.toFixed(2)} ${y0.toFixed(2)} A${r1} ${r1} 0 ${large} 1 ${x1.toFixed(2)} ${y1.toFixed(2)} L${x2.toFixed(2)} ${y2.toFixed(2)} A${r0} ${r0} 0 ${large} 0 ${x3.toFixed(2)} ${y3.toFixed(2)} Z`
}

export function FanChart() {
  const { t } = useTranslation()
  const { personId, select, go } = useApp()
  const [gens, setGens] = useState(5)
  const [span, setSpan] = useState(360)
  const [colorBy, setColorBy] = useState<'sex' | 'branch' | 'generation'>('branch')
  const [text, setText] = useState<'radial' | 'horizontal'>('radial')
  const svgRef = useRef<SVGSVGElement>(null)
  const { data } = useQuery({
    ...q<{ items: Item[] }>('chart.ahnentafel', { root: personId, generations: gens - 1 }),
    enabled: !!personId,
  })
  if (!personId) return <div className="p-10 text-center text-[var(--muted)]">{t('tree.pickRoot')}</div>
  const ringW = 78
  const hole = 46
  const R = hole + gens * ringW
  const size = R * 2 + 24
  const cx = size / 2
  const cy = span === 360 ? size / 2 : R + 12
  const height = span === 360 ? size : span === 180 ? R + 40 : R + 40
  const palette = ['#2f6f5e', '#4a8a9b', '#6a7fb0', '#b0709a']
  const items = data?.items ?? []
  const fill = (it: Item, g: number): string => {
    if (colorBy === 'sex') return sexColor(it.person.sex)
    if (colorBy === 'generation') return palette[g % palette.length]
    if (g === 0) return 'var(--accent)'
    const top = Math.floor(it.n / 2 ** (g - 2)) // 4..7 grandparent index at g>=2; 2..3 for parents
    return g === 1 ? (it.n === 2 ? palette[0] : palette[3]) : palette[Math.max(0, Math.min(3, top - 4))]
  }
  const startDeg = -span / 2
  return (
    <div className="flex h-full min-h-0 flex-col">
      <h1 className="sr-only">{t('nav.fan')}</h1>
      <div className="flex flex-wrap items-center gap-3 border-b border-[var(--border)] bg-[var(--surface)] px-3 py-2">
        <label className="flex items-center gap-1 text-sm">
          {t('fan.generations')}
          <input
            type="range"
            min={2}
            max={8}
            value={gens}
            onChange={(e) => setGens(Number(e.target.value))}
            aria-label={t('fan.generations')}
          />{' '}
          <span data-testid="fan-gens">{gens}</span>
        </label>
        <select
          className="input !w-auto"
          value={span}
          onChange={(e) => setSpan(Number(e.target.value))}
          aria-label={t('fan.span')}
        >
          <option value={360}>{t('fan.full')}</option>
          <option value={180}>{t('fan.half')}</option>
          <option value={90}>{t('fan.quarter')}</option>
        </select>
        <select
          className="input !w-auto"
          value={colorBy}
          onChange={(e) => setColorBy(e.target.value as typeof colorBy)}
          aria-label={t('tree.colorBy')}
        >
          <option value="branch">{t('fan.colorBranch')}</option>
          <option value="sex">{t('tree.colorSex')}</option>
          <option value="generation">{t('tree.colorGeneration')}</option>
        </select>
        <select
          className="input !w-auto"
          value={text}
          onChange={(e) => setText(e.target.value as typeof text)}
          aria-label={t('fan.text')}
        >
          <option value="radial">{t('fan.textRadial')}</option>
          <option value="horizontal">{t('fan.textHorizontal')}</option>
        </select>
        <div className="flex-1" />
        <button
          type="button"
          className="btn"
          onClick={() => svgRef.current && downloadBlob('fan-chart.svg', serializeSvg(svgRef.current), 'image/svg+xml')}
        >
          SVG
        </button>
        <button type="button" className="btn" onClick={() => window.print()}>
          {t('tree.print')}
        </button>
      </div>
      <div className="min-h-0 flex-1 overflow-auto p-4">
        <svg
          ref={svgRef}
          xmlns="http://www.w3.org/2000/svg"
          viewBox={`0 0 ${size} ${height}`}
          width="100%"
          style={{ maxHeight: '100%', maxWidth: 900, margin: '0 auto', display: 'block' }}
          role="img"
          aria-label={t('nav.fan')}
          data-testid="fan-svg"
        >
          {items.map((it) => {
            const g = Math.floor(Math.log2(it.n))
            const slots = 2 ** g
            const idx = it.n - slots
            const a0 = startDeg + (idx * span) / slots
            const a1 = startDeg + ((idx + 1) * span) / slots
            const r0 = g === 0 ? 0 : hole + (g - 1) * ringW
            const r1 = g === 0 ? hole : hole + g * ringW
            const mid = (a0 + a1) / 2
            const [tx, ty] = pt(cx, cy, g === 0 ? 0 : (r0 + r1) / 2, mid)
            // radial text reads outward; on the left half it is flipped so it is never upside down
            const flip = Math.sin(rad(mid)) < 0
            const rotation = text === 'horizontal' || g === 0 ? 0 : flip ? mid + 90 : mid - 90
            const name = it.person.name || t('person.unnamed')
            const maxChars = Math.max(
              6,
              Math.floor((text === 'radial' ? ringW : ((a1 - a0) * Math.PI * (r0 + r1)) / 360) / 6.6),
            )
            return (
              <g
                key={it.n}
                data-testid="fan-slot"
                data-n={it.n}
                style={{ cursor: 'pointer' }}
                onClick={(e) => {
                  if (e.shiftKey) {
                    select(it.person.id)
                    go('persons', it.person.id)
                  } else select(it.person.id)
                }}
                tabIndex={0}
                role="button"
                aria-label={`${it.n}. ${name}`}
                onKeyDown={(e) => e.key === 'Enter' && select(it.person.id)}
              >
                <path
                  d={sectorPath(cx, cy, r0, r1, a0, a1)}
                  fill={fill(it, g)}
                  fillOpacity={g === 0 ? 1 : 0.9 - g * 0.05}
                  stroke="var(--bg)"
                  strokeWidth={1.5}
                />
                <text
                  transform={`translate(${tx} ${ty}) rotate(${rotation})`}
                  textAnchor="middle"
                  dominantBaseline="middle"
                  fontSize={g > 4 ? 10 : 12}
                  fill="#fff"
                  style={{ pointerEvents: 'none' }}
                >
                  <tspan x={0} dy={g === 0 ? '-0.2em' : '-0.35em'} fontWeight={600}>
                    {name.length > maxChars ? name.slice(0, maxChars - 1) + '…' : name}
                  </tspan>
                  {g < 6 && (
                    <tspan x={0} dy="1.2em" fontSize={g > 3 ? 9 : 10.5} opacity={0.9}>
                      {it.person.life}
                    </tspan>
                  )}
                </text>
                <title>{`${it.n}. ${name} ${it.person.life}`}</title>
              </g>
            )
          })}
        </svg>
      </div>
    </div>
  )
}
