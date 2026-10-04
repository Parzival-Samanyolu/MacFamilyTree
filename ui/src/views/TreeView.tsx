import * as Menu from '@radix-ui/react-dropdown-menu'
import { useQuery } from '@tanstack/react-query'
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { Summary, TreeResult } from '../api/types'
import { Icons } from '../components/Icons'
import { Modal } from '../components/Modal'
import { downloadBlob, sexColor } from '../lib/format'
import { act, q } from '../lib/query'
import {
  centerOn,
  edgePath,
  fitTransform,
  nearest,
  visibleIndices,
  zoomAt,
  type Dir,
  type Transform,
} from '../lib/treeGeom'
import { useApp } from '../store/app'

interface Style {
  showDates: boolean
  showPlaces: boolean
  size: 'compact' | 'normal' | 'large'
  colorBy: 'sex' | 'generation' | 'custom' | 'none'
  rounded: boolean
}
const DEFAULT_STYLE: Style = { showDates: true, showPlaces: false, size: 'normal', colorBy: 'sex', rounded: true }
const SIZES = { compact: { w: 130, h: 44 }, normal: { w: 170, h: 62 }, large: { w: 210, h: 84 } }
const GEN_COLORS = ['#2f6f5e', '#4a8a9b', '#6a7fb0', '#8a73b0', '#b0709a', '#c27a6c', '#b59a4a', '#7a9a4a']

function loadStyles(): Record<string, Style> {
  try {
    return JSON.parse(localStorage.getItem('kt.treeStyles') ?? '{}')
  } catch {
    return {}
  }
}

function clip(s: string, max: number) {
  return s.length > max ? s.slice(0, Math.max(1, max - 1)) + '…' : s
}

/** Serialise an SVG element with CSS variables resolved, so exports look the same outside the app. */
export function serializeSvg(svg: SVGSVGElement): string {
  const clone = svg.cloneNode(true) as SVGSVGElement
  const cs = getComputedStyle(document.documentElement)
  let text = new XMLSerializer().serializeToString(clone)
  text = text.replace(/var\((--[a-z0-9-]+)\)/gi, (_m, name: string) => cs.getPropertyValue(name).trim() || '#000')
  return text
}

export function TreeView() {
  const { t } = useTranslation()
  const { personId, go, select, toast } = useApp()
  const root = personId
  const [mode, setMode] = useState<'hourglass' | 'ancestors' | 'descendants'>('hourglass')
  const [anc, setAnc] = useState(3)
  const [desc, setDesc] = useState(2)
  const [direction, setDirection] = useState<'td' | 'lr'>('td')
  const [spouses, setSpouses] = useState(true)
  const [collapsed, setCollapsed] = useState<string[]>([])
  const [style, setStyle] = useState<Style>(() => ({ ...DEFAULT_STYLE, ...(loadStyles()['__last'] ?? {}) }))
  const [styleName, setStyleName] = useState('')
  const [selected, setSelected] = useState<number[]>([])
  const [menu, setMenu] = useState<{ x: number; y: number; idx: number } | null>(null)
  const [t2, setT] = useState<Transform>({ k: 1, tx: 0, ty: 0 })
  const [size, setSize] = useState({ w: 800, h: 600 })
  const [styleOpen, setStyleOpen] = useState(false)
  const wrap = useRef<HTMLDivElement>(null)
  const svgRef = useRef<SVGSVGElement>(null)
  const drag = useRef<{ x: number; y: number; moved: boolean } | null>(null)
  const [dragging, setDragging] = useState(false)
  const fitted = useRef<string>('')

  const dim = SIZES[style.size]
  const args = {
    root,
    mode,
    ancestors: anc,
    descendants: desc,
    direction,
    show_spouses: spouses,
    collapsed,
    card_w: dim.w,
    card_h: dim.h,
    show_places: style.showPlaces,
  }
  const { data, error, isFetching } = useQuery({
    ...q<TreeResult>('tree.layout', args),
    enabled: !!root,
    placeholderData: (prev) => prev,
  })
  const layout = data?.layout
  const people = useMemo(() => data?.people ?? {}, [data])
  const vertical = direction === 'td'

  useLayoutEffect(() => {
    const el = wrap.current
    if (!el) return
    const ro = new ResizeObserver(() => setSize({ w: el.clientWidth, h: el.clientHeight }))
    ro.observe(el)
    setSize({ w: el.clientWidth, h: el.clientHeight })
    return () => ro.disconnect()
  }, [])

  const fit = useCallback(() => {
    if (layout) setT(fitTransform(layout.width, layout.height, size.w, size.h))
  }, [layout, size.w, size.h])
  // fit once per (root, mode, direction) so later edits keep the user's pan/zoom
  useEffect(() => {
    if (!layout) return
    const key = `${root}|${mode}|${direction}|${anc}|${desc}`
    if (fitted.current !== key) {
      fitted.current = key
      const rootNode = layout.nodes[0]
      const f = fitTransform(layout.width, layout.height, size.w, size.h)
      // very large charts open centred on the root at a readable zoom instead of an unreadable fit
      setT(
        f.k < 0.35 && rootNode
          ? centerOn({ ...f, k: 0.6 }, rootNode.x + rootNode.w / 2, rootNode.y + rootNode.h / 2, size.w, size.h)
          : f,
      )
    }
  }, [layout, root, mode, direction, anc, desc, size.w, size.h])

  const rootIdx = layout ? layout.nodes.findIndex((n) => n.person_id === root && !n.is_spouse && n.dup_of === null) : -1
  const sel = selected[0] ?? rootIdx
  const visible = useMemo(() => (layout ? visibleIndices(layout.nodes, t2, size.w, size.h) : []), [layout, t2, size])

  const setRootTo = (pid: string) => {
    select(pid)
    setSelected([])
  }
  const edit = (pid: string) => {
    select(pid)
    go('persons', pid)
  }
  const color = (idx: number): string => {
    const n = layout!.nodes[idx]
    const p = people[n.person_id]
    if (style.colorBy === 'none') return 'var(--accent)'
    if (style.colorBy === 'generation') return GEN_COLORS[Math.abs(n.generation) % GEN_COLORS.length]
    if (style.colorBy === 'custom' && p?.color) return p.color
    return sexColor(p?.sex ?? '')
  }

  const onWheel = (e: React.WheelEvent) => {
    const r = wrap.current!.getBoundingClientRect()
    if (e.ctrlKey || e.metaKey || Math.abs(e.deltaY) >= Math.abs(e.deltaX))
      setT((t) => zoomAt(t, e.clientX - r.left, e.clientY - r.top, Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0015))))
    else setT((t) => ({ ...t, tx: t.tx - e.deltaX }))
  }
  const onPointerDown = (e: React.PointerEvent) => {
    // cards, the context menu and any control keep their own clicks (pointer capture would retarget them)
    if ((e.target as HTMLElement).closest('[data-card],[role="menu"],button,select,input,a')) return
    drag.current = { x: e.clientX, y: e.clientY, moved: false }
    setDragging(true)
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  }
  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current
    if (!d) return
    const dx = e.clientX - d.x
    const dy = e.clientY - d.y
    if (Math.abs(dx) + Math.abs(dy) > 2) d.moved = true
    d.x = e.clientX
    d.y = e.clientY
    setT((t) => ({ ...t, tx: t.tx + dx, ty: t.ty + dy }))
  }
  const onPointerUp = () => {
    drag.current = null
    setDragging(false)
  }

  const focusNode = (idx: number) => {
    setSelected([idx])
    const n = layout!.nodes[idx]
    const cx = n.x + n.w / 2
    const cy = n.y + n.h / 2
    const sx = cx * t2.k + t2.tx
    const sy = cy * t2.k + t2.ty
    if (sx < 60 || sx > size.w - 60 || sy < 60 || sy > size.h - 60) setT(centerOn(t2, cx, cy, size.w, size.h))
  }
  const onKey = (e: React.KeyboardEvent) => {
    if (!layout) return
    const map: Record<string, Dir> = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right' }
    if (map[e.key]) {
      e.preventDefault()
      const nx = nearest(layout.nodes, sel < 0 ? 0 : sel, map[e.key])
      if (nx !== null) focusNode(nx)
    } else if ((e.key === 'Enter' || e.key.toLowerCase() === 'e') && sel >= 0) {
      edit(layout.nodes[sel].person_id)
    } else if (e.key === 'r' && sel >= 0) setRootTo(layout.nodes[sel].person_id)
    else if (e.key === '+' || e.key === '=') setT((t) => zoomAt(t, size.w / 2, size.h / 2, 1.2))
    else if (e.key === '-') setT((t) => zoomAt(t, size.w / 2, size.h / 2, 1 / 1.2))
    else if (e.key === '0') fit()
    else if (e.key === 'Escape') {
      setMenu(null)
      setSelected([])
    }
  }

  const exportSvg = () => {
    if (svgRef.current) downloadBlob('tree.svg', serializeSvg(svgRef.current), 'image/svg+xml')
  }
  const exportPng = async () => {
    if (!svgRef.current || !layout) return
    const scale = 2
    const clone = svgRef.current.cloneNode(true) as SVGSVGElement
    clone.setAttribute('width', String(layout.width * scale))
    clone.setAttribute('height', String(layout.height * scale))
    clone.setAttribute('viewBox', `0 0 ${layout.width} ${layout.height}`)
    clone.querySelector('[data-viewport]')?.removeAttribute('transform')
    const bg = getComputedStyle(document.documentElement).getPropertyValue('--bg').trim() || '#fff'
    const text = serializeSvg(clone).replace('<svg', `<svg style="background:${bg}"`)
    const img = new Image()
    img.onload = () => {
      const c = document.createElement('canvas')
      c.width = layout.width * scale
      c.height = layout.height * scale
      const ctx = c.getContext('2d')!
      ctx.fillStyle = bg
      ctx.fillRect(0, 0, c.width, c.height)
      ctx.drawImage(img, 0, 0)
      c.toBlob((b) => b && downloadBlob('tree.png', b, 'image/png'))
    }
    img.onerror = () => toast(t('tree.exportFailed'), 'error')
    img.src = 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(text)
  }

  const saveStyle = () => {
    const name = styleName.trim()
    if (!name) return
    const all = loadStyles()
    all[name] = style
    localStorage.setItem('kt.treeStyles', JSON.stringify(all))
    setStyleName('')
    toast(t('tree.styleSaved', { name }))
  }
  const savedStyles = loadStyles()
  useEffect(() => {
    const all = loadStyles()
    all['__last'] = style
    try {
      localStorage.setItem('kt.treeStyles', JSON.stringify(all))
    } catch {
      /* ignore */
    }
  }, [style])

  if (!root) return <div className="p-10 text-center text-[var(--muted)]">{t('tree.pickRoot')}</div>
  if (error) return <div className="p-6 text-[var(--danger)]">{String((error as Error).message)}</div>

  const minimapScale = layout ? Math.min(160 / layout.width, 110 / layout.height) : 1
  const num = (label: string, v: number, set: (n: number) => void, max = 12) => (
    <label className="flex items-center gap-1 text-sm">
      {label}
      <button type="button" className="btn !px-2" onClick={() => set(Math.max(0, v - 1))} aria-label={`${label} −`}>
        −
      </button>
      <span className="w-5 text-center" data-testid={`gen-${label}`}>
        {v}
      </span>
      <button type="button" className="btn !px-2" onClick={() => set(Math.min(max, v + 1))} aria-label={`${label} +`}>
        +
      </button>
    </label>
  )

  return (
    <div className="flex h-full min-h-0 flex-col">
      <h1 className="sr-only">{t('nav.tree')}</h1>
      <div
        className="flex flex-wrap items-center gap-3 border-b border-[var(--border)] bg-[var(--surface)] px-3 py-2"
        role="toolbar"
        aria-label={t('tree.toolbar')}
      >
        <select
          className="input !w-auto"
          value={mode}
          onChange={(e) => setMode(e.target.value as typeof mode)}
          aria-label={t('tree.mode')}
        >
          <option value="hourglass">{t('tree.hourglass')}</option>
          <option value="ancestors">{t('tree.ancestors')}</option>
          <option value="descendants">{t('tree.descendants')}</option>
        </select>
        {mode !== 'descendants' && num(t('tree.gensUp'), anc, setAnc)}
        {mode !== 'ancestors' && num(t('tree.gensDown'), desc, setDesc)}
        <select
          className="input !w-auto"
          value={direction}
          onChange={(e) => setDirection(e.target.value as 'td' | 'lr')}
          aria-label={t('tree.direction')}
        >
          <option value="td">{t('tree.topDown')}</option>
          <option value="lr">{t('tree.leftRight')}</option>
        </select>
        <label className="flex items-center gap-1 text-sm">
          <input type="checkbox" checked={spouses} onChange={(e) => setSpouses(e.target.checked)} /> {t('tree.spouses')}
        </label>
        <button type="button" className="btn" onClick={() => setStyleOpen(true)}>
          {t('tree.cardStyle')}
        </button>
        <div className="flex-1" />
        <button
          type="button"
          className="btn"
          onClick={() => setT((x) => zoomAt(x, size.w / 2, size.h / 2, 1 / 1.2))}
          aria-label={t('tree.zoomOut')}
        >
          −
        </button>
        <span className="w-12 text-center text-sm tabular-nums" data-testid="zoom-level">
          {Math.round(t2.k * 100)}%
        </span>
        <button
          type="button"
          className="btn"
          onClick={() => setT((x) => zoomAt(x, size.w / 2, size.h / 2, 1.2))}
          aria-label={t('tree.zoomIn')}
        >
          +
        </button>
        <button type="button" className="btn" onClick={fit}>
          {t('tree.fit')}
        </button>
        <Menu.Root>
          <Menu.Trigger className="btn">{t('tree.export')}</Menu.Trigger>
          <Menu.Portal>
            <Menu.Content className="card z-50 p-1" sideOffset={6}>
              <Menu.Item
                className="cursor-pointer rounded-md px-3 py-1.5 text-sm outline-none data-[highlighted]:bg-[var(--surface-2)]"
                onSelect={() => exportSvg()}
              >
                SVG
              </Menu.Item>
              <Menu.Item
                className="cursor-pointer rounded-md px-3 py-1.5 text-sm outline-none data-[highlighted]:bg-[var(--surface-2)]"
                onSelect={() => void exportPng()}
              >
                PNG
              </Menu.Item>
              <Menu.Item
                className="cursor-pointer rounded-md px-3 py-1.5 text-sm outline-none data-[highlighted]:bg-[var(--surface-2)]"
                onSelect={() => window.print()}
              >
                {t('tree.print')}
              </Menu.Item>
            </Menu.Content>
          </Menu.Portal>
        </Menu.Root>
      </div>

      <div
        ref={wrap}
        className="relative min-h-0 flex-1 overflow-hidden bg-[var(--bg)] outline-none"
        tabIndex={0}
        onKeyDown={onKey}
        onWheel={onWheel}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onClick={() => setMenu(null)}
        style={{ cursor: dragging ? 'grabbing' : 'grab', touchAction: 'none' }}
        data-testid="tree-canvas"
        aria-label={t('tree.canvas')}
        role="application"
      >
        {layout && (
          <svg
            ref={svgRef}
            width="100%"
            height="100%"
            xmlns="http://www.w3.org/2000/svg"
            role="img"
            aria-label={t('tree.canvas')}
          >
            <g data-viewport transform={`translate(${t2.tx} ${t2.ty}) scale(${t2.k})`}>
              <g fill="none" stroke="var(--border)" strokeWidth={1.6}>
                {layout.edges.map((e, i) => {
                  const touches = (idx: number) => visible.includes(idx)
                  const near = e.from_union
                    ? touches(e.to)
                    : e.to_union
                      ? touches(e.from)
                      : touches(e.from) || touches(e.to)
                  if (!near) return null
                  const dashed = e.link === 'Adopted' || e.link === 'Foster' || e.link === 'Step'
                  const ended = e.kind === 'Partner' && (e.link === 'divorced' || e.link === 'separated')
                  return (
                    <path
                      key={i}
                      d={edgePath(layout, e, vertical)}
                      strokeDasharray={dashed ? '5 4' : ended ? '1 5' : undefined}
                      strokeLinecap={ended ? 'round' : undefined}
                      data-ended={ended ? '1' : undefined}
                      stroke={e.kind === 'Partner' ? 'var(--accent)' : 'var(--muted)'}
                      opacity={e.kind === 'Partner' ? 0.7 : 0.8}
                    />
                  )
                })}
                {layout.unions.map((u, i) => (
                  <circle key={i} cx={u.x} cy={u.y} r={3} fill="var(--accent)" stroke="none" />
                ))}
              </g>
              {visible.map((i) => {
                const n = layout.nodes[i]
                const p: Summary | undefined = people[n.person_id]
                const isSel = selected.includes(i) || (selected.length === 0 && i === rootIdx)
                const chars = Math.floor((n.w - 26) / 7)
                const dupNode = n.dup_of !== null
                return (
                  <g
                    key={`${i}-${n.person_id}`}
                    data-card
                    data-testid="tree-card"
                    data-person={n.person_id}
                    data-root={i === rootIdx ? '1' : undefined}
                    style={{
                      transform: `translate(${n.x}px, ${n.y}px)`,
                      transition: 'transform 250ms ease',
                      cursor: 'pointer',
                    }}
                    onClick={(e) => {
                      e.stopPropagation()
                      setSelected(e.shiftKey ? [...selected, i] : [i])
                      setMenu(null)
                      wrap.current?.focus()
                    }}
                    onDoubleClick={(e) => {
                      e.stopPropagation()
                      edit(n.person_id)
                    }}
                    onContextMenu={(e) => {
                      e.preventDefault()
                      e.stopPropagation()
                      const r = wrap.current!.getBoundingClientRect()
                      setSelected([i])
                      setMenu({ x: e.clientX - r.left, y: e.clientY - r.top, idx: i })
                    }}
                  >
                    <rect
                      width={n.w}
                      height={n.h}
                      rx={style.rounded ? 10 : 2}
                      fill="var(--surface)"
                      stroke={isSel ? 'var(--accent)' : 'var(--border)'}
                      strokeWidth={isSel ? 3 : 1.2}
                      strokeDasharray={dupNode ? '5 3' : undefined}
                    />
                    <rect width={6} height={n.h} rx={style.rounded ? 3 : 0} fill={color(i)} />
                    <text x={16} y={22} fontSize={13} fontWeight={600} fill="var(--text)">
                      {clip(p?.name || t('person.unnamed'), chars)}
                    </text>
                    {style.showDates && (
                      <text x={16} y={40} fontSize={11} fill="var(--muted)">
                        {p?.life ?? ''}
                        {dupNode ? ' ↺' : ''}
                      </text>
                    )}
                    {style.showPlaces && n.h >= 62 && (
                      <text x={16} y={55} fontSize={10} fill="var(--muted)">
                        {clip((p as (Summary & { birth_place?: string }) | undefined)?.birth_place ?? '', chars + 2)}
                      </text>
                    )}
                    {n.has_more && (
                      <text x={n.w - 14} y={n.h - 8} fontSize={14} fill="var(--accent)" aria-label={t('tree.hasMore')}>
                        …
                      </text>
                    )}
                    <title>
                      {p?.name} {p?.life}
                    </title>
                  </g>
                )
              })}
            </g>
          </svg>
        )}
        {isFetching && <div className="absolute left-3 top-3 text-xs text-[var(--muted)]">…</div>}

        {layout && (
          <svg
            className="card absolute bottom-3 right-3"
            width={176}
            height={126}
            aria-hidden="true"
            onPointerDown={(e) => e.stopPropagation()}
            onClick={(e) => {
              e.stopPropagation()
              const r = (e.currentTarget as SVGSVGElement).getBoundingClientRect()
              const cx = (e.clientX - r.left - 8) / minimapScale
              const cy = (e.clientY - r.top - 8) / minimapScale
              setT(centerOn(t2, cx, cy, size.w, size.h))
            }}
          >
            <g transform={`translate(8 8) scale(${minimapScale})`}>
              {layout.nodes.map((n, i) => (
                <rect
                  key={i}
                  x={n.x}
                  y={n.y}
                  width={n.w}
                  height={n.h}
                  fill={i === sel ? 'var(--accent)' : 'var(--muted)'}
                  opacity={0.6}
                />
              ))}
              <rect
                x={-t2.tx / t2.k}
                y={-t2.ty / t2.k}
                width={size.w / t2.k}
                height={size.h / t2.k}
                fill="none"
                stroke="var(--accent)"
                strokeWidth={2 / minimapScale}
              />
            </g>
          </svg>
        )}

        {menu && layout && (
          <div
            className="card absolute z-30 min-w-44 p-1"
            style={{ left: menu.x, top: menu.y }}
            role="menu"
            onClick={(e) => e.stopPropagation()}
            data-testid="tree-menu"
          >
            {(() => {
              const pid = layout.nodes[menu.idx].person_id
              const item = (label: string, run: () => void) => (
                <button
                  type="button"
                  role="menuitem"
                  className="block w-full rounded-md px-3 py-1.5 text-left text-sm hover:bg-[var(--surface-2)]"
                  onClick={() => {
                    run()
                    setMenu(null)
                  }}
                >
                  {label}
                </button>
              )
              return (
                <>
                  {item(t('tree.edit'), () => edit(pid))}
                  {item(t('tree.setRoot'), () => setRootTo(pid))}
                  {item(collapsed.includes(pid) ? t('tree.expand') : t('tree.collapse'), () =>
                    setCollapsed((c) => (c.includes(pid) ? c.filter((x) => x !== pid) : [...c, pid])),
                  )}
                  {layout.nodes[menu.idx].dup_of !== null &&
                    item(t('tree.jumpToOriginal'), () => focusNode(layout.nodes[menu.idx].dup_of!))}
                  {item(
                    t('person.addRelative') + ' – ' + t('relative.child'),
                    () => void act('relative.add', { person_id: pid, kind: 'child', given: '' }),
                  )}
                  {item(
                    t('person.addRelative') + ' – ' + t('relative.partner'),
                    () => void act('relative.add', { person_id: pid, kind: 'partner', given: '' }),
                  )}
                  {item(t('tree.copyName'), () => void navigator.clipboard?.writeText(people[pid]?.name ?? ''))}
                </>
              )
            })()}
          </div>
        )}
      </div>

      <Modal open={styleOpen} onOpenChange={setStyleOpen} title={t('tree.cardStyle')}>
        <div className="grid gap-3 sm:grid-cols-2">
          <div>
            <label className="label" htmlFor="st-size">
              {t('tree.cardSize')}
            </label>
            <select
              id="st-size"
              className="input"
              value={style.size}
              onChange={(e) => setStyle({ ...style, size: e.target.value as Style['size'] })}
            >
              <option value="compact">{t('tree.sizeCompact')}</option>
              <option value="normal">{t('tree.sizeNormal')}</option>
              <option value="large">{t('tree.sizeLarge')}</option>
            </select>
          </div>
          <div>
            <label className="label" htmlFor="st-color">
              {t('tree.colorBy')}
            </label>
            <select
              id="st-color"
              className="input"
              value={style.colorBy}
              onChange={(e) => setStyle({ ...style, colorBy: e.target.value as Style['colorBy'] })}
            >
              <option value="sex">{t('tree.colorSex')}</option>
              <option value="generation">{t('tree.colorGeneration')}</option>
              <option value="custom">{t('tree.colorCustom')}</option>
              <option value="none">{t('tree.colorNone')}</option>
            </select>
          </div>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={style.showDates}
              onChange={(e) => setStyle({ ...style, showDates: e.target.checked })}
            />{' '}
            {t('tree.showDates')}
          </label>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={style.showPlaces}
              onChange={(e) => setStyle({ ...style, showPlaces: e.target.checked })}
            />{' '}
            {t('tree.showPlaces')}
          </label>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={style.rounded}
              onChange={(e) => setStyle({ ...style, rounded: e.target.checked })}
            />{' '}
            {t('tree.rounded')}
          </label>
        </div>
        <div className="mt-4 border-t border-[var(--border)] pt-3">
          <div className="mb-2 text-sm font-medium">{t('tree.savedStyles')}</div>
          <div className="flex flex-wrap gap-2">
            {Object.keys(savedStyles)
              .filter((k) => k !== '__last')
              .map((k) => (
                <button
                  key={k}
                  type="button"
                  className="btn"
                  onClick={() => setStyle({ ...DEFAULT_STYLE, ...savedStyles[k] })}
                >
                  {k}
                </button>
              ))}
          </div>
          <div className="mt-2 flex gap-2">
            <input
              className="input"
              aria-label={t('tree.styleName')}
              placeholder={t('tree.styleName')}
              value={styleName}
              onChange={(e) => setStyleName(e.target.value)}
            />
            <button type="button" className="btn" onClick={saveStyle} disabled={!styleName.trim()}>
              {Icons.plus} {t('common.save')}
            </button>
          </div>
        </div>
      </Modal>
    </div>
  )
}
