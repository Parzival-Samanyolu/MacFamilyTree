import { useQuery } from '@tanstack/react-query'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import * as THREE from 'three'
import { OrbitControls } from 'three/addons/controls/OrbitControls.js'
import { mergeGeometries } from 'three/addons/utils/BufferGeometryUtils.js'
import type { Summary, TreeResult } from '../api/types'
import { downloadBlob } from '../lib/format'
import { q } from '../lib/query'
import { place3d, THEMES, type Theme } from '../lib/virtual3d'
import { useApp } from '../store/app'

const MAX_LABELS = 90
const TUBE_EDGE_LIMIT = 700

interface Ctx {
  renderer: THREE.WebGLRenderer
  scene: THREE.Scene
  camera: THREE.PerspectiveCamera
  controls: OrbitControls
  content: THREE.Group
  labels: THREE.Group
  nodeMesh: THREE.InstancedMesh | null
  positions: THREE.Vector3[]
  people: Summary[]
  radius: number
  theme: Theme
  dirty: boolean
  tween: { t0: number; from: [THREE.Vector3, THREE.Vector3]; to: [THREE.Vector3, THREE.Vector3] } | null
  labelCache: Map<string, THREE.Sprite>
}

function labelSprite(text: string, sub: string, theme: Theme): THREE.Sprite {
  const c = document.createElement('canvas')
  c.width = 384
  c.height = 112
  const g = c.getContext('2d')!
  g.fillStyle = theme.labelBg
  g.beginPath()
  g.roundRect(4, 4, c.width - 8, c.height - 8, 18)
  g.fill()
  g.fillStyle = theme.label
  g.textAlign = 'center'
  g.font = '600 38px system-ui, sans-serif'
  g.fillText(text.length > 22 ? `${text.slice(0, 21)}…` : text, c.width / 2, 52, c.width - 24)
  g.font = '28px system-ui, sans-serif'
  g.fillText(sub, c.width / 2, 92, c.width - 24)
  const tex = new THREE.CanvasTexture(c)
  tex.colorSpace = THREE.SRGBColorSpace
  const sp = new THREE.Sprite(new THREE.SpriteMaterial({ map: tex, depthTest: false, transparent: true }))
  sp.scale.set(7.2, 2.1, 1)
  sp.renderOrder = 10
  return sp
}

function disposeGroup(g: THREE.Group) {
  g.traverse((o) => {
    const m = o as THREE.Mesh
    m.geometry?.dispose()
    const mat = m.material as THREE.Material | THREE.Material[] | undefined
    ;[mat].flat().forEach((x) => {
      if (!x) return
      ;(x as THREE.SpriteMaterial).map?.dispose()
      x.dispose()
    })
  })
  g.clear()
}

export function VirtualTree() {
  const { t } = useTranslation()
  const { personId, select, go } = useApp()
  const host = useRef<HTMLDivElement>(null)
  const ctx = useRef<Ctx | null>(null)
  const [mode, setMode] = useState<'hourglass' | 'ancestors' | 'descendants'>('hourglass')
  const [gens, setGens] = useState(5)
  const [themeId, setThemeId] = useState<Theme['id']>('garden')
  const [spin, setSpin] = useState(false)
  const [showLabels, setShowLabels] = useState(true)
  const [webgl, setWebgl] = useState(true)
  const [ready, setReady] = useState(0)
  const [hover, setHover] = useState<{ x: number; y: number; p: Summary } | null>(null)
  const [focus, setFocus] = useState<string | null>(null)
  const [labelCount, setLabelCount] = useState(0)
  const theme = THEMES[themeId]

  const { data } = useQuery({
    ...q<TreeResult>('tree.layout', {
      root: personId,
      mode,
      ancestors: gens,
      descendants: Math.max(1, gens - 2),
      show_spouses: true,
      card_w: 100,
      card_h: 50,
    }),
    enabled: !!personId,
    placeholderData: (prev) => prev,
  })
  const people = useMemo(() => (data ? data.layout.nodes.map((n) => data.people[n.person_id]) : []), [data])

  const showLabelsRef = useRef(showLabels)
  const labelCountRef = useRef(0)
  const updateLabels = useCallback((c: Ctx) => {
    const want = new Set<string>()
    if (showLabelsRef.current) {
      const cam = c.camera.position
      const range = Math.max(18, c.radius * 0.7)
      const near = c.positions
        .map((p, i) => ({ i, d: p.distanceTo(cam) }))
        .filter((x) => x.d < range * 1.6)
        .sort((a, b) => a.d - b.d)
        .slice(0, MAX_LABELS)
      for (const { i } of near) {
        const person = c.people[i]
        if (!person) continue
        const key = `${person.id}:${i}`
        want.add(key)
        let sp = c.labelCache.get(key)
        if (!sp) {
          sp = labelSprite(person.name || '?', person.life, c.theme)
          sp.userData.themeId = c.theme.id
          c.labelCache.set(key, sp)
          c.labels.add(sp)
        }
        sp.position.copy(c.positions[i]).add(new THREE.Vector3(0, 1.9, 0))
        sp.visible = true
      }
    }
    for (const [k, sp] of c.labelCache) {
      if (!want.has(k)) {
        c.labels.remove(sp)
        sp.material.map?.dispose()
        sp.material.dispose()
        c.labelCache.delete(k)
      }
    }
    labelCountRef.current = want.size
  }, [])
  useEffect(() => {
    showLabelsRef.current = showLabels
    if (ctx.current) ctx.current.dirty = true
  }, [showLabels])
  useEffect(() => {
    const h = setInterval(() => setLabelCount(labelCountRef.current), 300)
    return () => clearInterval(h)
  }, [])

  // renderer, camera, controls
  useEffect(() => {
    const el = host.current
    if (!el) return
    let renderer: THREE.WebGLRenderer
    try {
      renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true })
    } catch {
      queueMicrotask(() => setWebgl(false))
      return
    }
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
    renderer.setSize(el.clientWidth, el.clientHeight)
    el.appendChild(renderer.domElement)
    renderer.domElement.setAttribute('role', 'img')
    renderer.domElement.setAttribute('aria-label', t('virtual.aria'))
    const scene = new THREE.Scene()
    const camera = new THREE.PerspectiveCamera(50, el.clientWidth / Math.max(1, el.clientHeight), 0.1, 2000)
    camera.position.set(0, 4, 30)
    const controls = new OrbitControls(camera, renderer.domElement)
    controls.enableDamping = true
    controls.dampingFactor = 0.08
    controls.autoRotateSpeed = 0.8
    const content = new THREE.Group()
    const labels = new THREE.Group()
    scene.add(content, labels)
    scene.add(new THREE.HemisphereLight(0xffffff, 0x445544, 1.6))
    const sun = new THREE.DirectionalLight(0xffffff, 1.4)
    sun.position.set(10, 20, 15)
    scene.add(sun)
    const c: Ctx = {
      renderer,
      scene,
      camera,
      controls,
      content,
      labels,
      nodeMesh: null,
      positions: [],
      people: [],
      radius: 20,
      theme: THEMES.garden,
      dirty: true,
      tween: null,
      labelCache: new Map(),
    }
    ctx.current = c
    controls.addEventListener('change', () => (c.dirty = true))
    const ro = new ResizeObserver(() => {
      renderer.setSize(el.clientWidth, el.clientHeight)
      camera.aspect = el.clientWidth / Math.max(1, el.clientHeight)
      camera.updateProjectionMatrix()
      c.dirty = true
    })
    ro.observe(el)
    const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    renderer.setAnimationLoop((now) => {
      if (c.tween) {
        const k = reduce ? 1 : Math.min(1, (now - c.tween.t0) / 600)
        const e = k * k * (3 - 2 * k)
        camera.position.lerpVectors(c.tween.from[0], c.tween.to[0], e)
        controls.target.lerpVectors(c.tween.from[1], c.tween.to[1], e)
        if (k >= 1) c.tween = null
        c.dirty = true
      }
      const moved = controls.update()
      if (moved || c.dirty) {
        updateLabels(c)
        renderer.render(scene, camera)
        c.dirty = false
      }
    })
    queueMicrotask(() => setReady((n) => n + 1))
    return () => {
      renderer.setAnimationLoop(null)
      ro.disconnect()
      disposeGroup(content)
      disposeGroup(labels)
      c.labelCache.clear()
      controls.dispose()
      renderer.dispose()
      renderer.domElement.remove()
      ctx.current = null
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // (re)build the scene content when the data or theme change
  useEffect(() => {
    const c = ctx.current
    if (!c || !data) return
    const th = THEMES[themeId]
    c.theme = th
    disposeGroup(c.content)
    for (const sp of c.labelCache.values()) {
      sp.material.map?.dispose()
      sp.material.dispose()
    }
    c.labelCache.clear()
    c.labels.clear()
    c.scene.background = new THREE.Color(th.background)
    c.scene.fog = new THREE.Fog(th.fog, 60, 600)
    const rootIdx = Math.max(
      0,
      data.layout.nodes.findIndex((n) => n.person_id === personId && n.dup_of == null),
    )
    const placed = place3d(data.layout, rootIdx, { up: mode !== 'descendants' })
    const pos = placed.nodes.map((p) => new THREE.Vector3(...p))
    c.positions = pos
    c.people = data.layout.nodes.map((n) => data.people[n.person_id])
    c.radius = placed.radius

    // people
    const geo = new THREE.SphereGeometry(0.85, 20, 14)
    const mat = new THREE.MeshStandardMaterial({
      roughness: 0.55,
      metalness: 0.05,
      emissive: new THREE.Color(th.id === 'night' ? 0x223344 : 0x000000),
    })
    const mesh = new THREE.InstancedMesh(geo, mat, pos.length)
    const m4 = new THREE.Matrix4()
    pos.forEach((p, i) => {
      const person = c.people[i]
      const isRoot = i === rootIdx
      m4.makeScale(isRoot ? 1.6 : 1, isRoot ? 1.6 : 1, isRoot ? 1.6 : 1).setPosition(p)
      mesh.setMatrixAt(i, m4)
      mesh.setColorAt(i, new THREE.Color(person?.sex === 'M' ? th.male : person?.sex === 'F' ? th.female : th.other))
    })
    mesh.instanceMatrix.needsUpdate = true
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true
    mesh.computeBoundingSphere()
    c.nodeMesh = mesh
    c.content.add(mesh)

    // unions
    const unionPos = placed.unions.map((p) => new THREE.Vector3(...p))
    if (unionPos.length) {
      const um = new THREE.InstancedMesh(
        new THREE.SphereGeometry(0.38, 10, 8),
        new THREE.MeshStandardMaterial({ color: th.union }),
        unionPos.length,
      )
      unionPos.forEach((p, i) => um.setMatrixAt(i, m4.makeTranslation(p.x, p.y, p.z)))
      um.instanceMatrix.needsUpdate = true
      c.content.add(um)
    }

    // branches
    const at = (i: number, union: boolean) => (union ? unionPos[i] : pos[i])
    const curves = data.layout.edges
      .map((e) => {
        const a = at(e.from, e.from_union)
        const b = at(e.to, e.to_union)
        if (!a || !b) return null
        const mid = a.clone().add(b).multiplyScalar(0.5)
        mid.y += (b.y - a.y) * 0.1
        mid.z += (jitterOf(e.link) * Math.abs(a.x - b.x)) / 12
        return new THREE.QuadraticBezierCurve3(a, mid, b)
      })
      .filter((x): x is THREE.QuadraticBezierCurve3 => !!x)
    if (curves.length <= TUBE_EDGE_LIMIT) {
      const tubes = curves.map((cv) => new THREE.TubeGeometry(cv, 10, 0.16, 5, false))
      if (tubes.length) {
        const merged = mergeGeometries(tubes)
        tubes.forEach((x) => x.dispose())
        c.content.add(new THREE.Mesh(merged, new THREE.MeshStandardMaterial({ color: th.branch, roughness: 0.9 })))
      }
    } else {
      const pts: THREE.Vector3[] = []
      for (const cv of curves) {
        const s = cv.getPoints(8)
        for (let i = 0; i < s.length - 1; i++) pts.push(s[i], s[i + 1])
      }
      c.content.add(
        new THREE.LineSegments(
          new THREE.BufferGeometry().setFromPoints(pts),
          new THREE.LineBasicMaterial({ color: th.branch }),
        ),
      )
    }

    // frame the camera on first load of a new root
    const r = Math.max(12, placed.radius)
    c.controls.target.set(0, 0, 0)
    c.camera.position.set(0, r * 0.15, r * 1.5)
    c.camera.far = r * 12
    c.camera.updateProjectionMatrix()
    c.dirty = true
    queueMicrotask(() => setReady((n) => n + 1))
  }, [data, themeId, personId, mode])

  useEffect(() => {
    const c = ctx.current
    if (!c) return
    c.controls.autoRotate = spin && !window.matchMedia('(prefers-reduced-motion: reduce)').matches
    c.dirty = true
  }, [spin])

  const flyTo = useCallback((i: number) => {
    const c = ctx.current
    if (!c || !c.positions[i]) return
    const target = c.positions[i].clone()
    const dir = c.camera.position.clone().sub(c.controls.target).normalize()
    c.tween = {
      t0: performance.now(),
      from: [c.camera.position.clone(), c.controls.target.clone()],
      to: [target.clone().add(dir.multiplyScalar(14)), target],
    }
  }, [])

  const resetCamera = () => {
    const c = ctx.current
    if (!c) return
    const r = Math.max(12, c.radius)
    c.tween = {
      t0: performance.now(),
      from: [c.camera.position.clone(), c.controls.target.clone()],
      to: [new THREE.Vector3(0, r * 0.15, r * 1.5), new THREE.Vector3(0, 0, 0)],
    }
  }

  const pick = (ev: React.PointerEvent | React.MouseEvent): number => {
    const c = ctx.current
    const el = host.current
    if (!c || !el || !c.nodeMesh) return -1
    const r = el.getBoundingClientRect()
    const ndc = new THREE.Vector2(((ev.clientX - r.left) / r.width) * 2 - 1, -((ev.clientY - r.top) / r.height) * 2 + 1)
    const ray = new THREE.Raycaster()
    ray.setFromCamera(ndc, c.camera)
    const hit = ray.intersectObject(c.nodeMesh)[0]
    return hit?.instanceId ?? -1
  }

  const shot = () => {
    const c = ctx.current
    if (!c) return
    c.renderer.render(c.scene, c.camera)
    c.renderer.domElement.toBlob((b) => b && downloadBlob('kintree-virtual-tree.png', b, 'image/png'))
  }

  if (!personId) return <div className="p-10 text-center text-[var(--muted)]">{t('tree.pickRoot')}</div>
  const uniq = data ? data.layout.nodes.filter((n) => n.dup_of == null) : []
  return (
    <div className="flex h-full flex-col p-4">
      <div className="mb-3 flex flex-wrap items-end gap-x-4 gap-y-2">
        <h1 className="text-2xl font-semibold tracking-tight">{t('nav.virtual')}</h1>
        <label className="text-sm">
          <span className="label">{t('tree.mode')}</span>
          <select
            className="input !w-auto"
            value={mode}
            onChange={(e) => setMode(e.target.value as typeof mode)}
            data-testid="v3-mode"
          >
            {(['hourglass', 'ancestors', 'descendants'] as const).map((m) => (
              <option key={m} value={m}>
                {t(`tree.${m}`)}
              </option>
            ))}
          </select>
        </label>
        <label className="text-sm">
          <span className="label">{t('virtual.generations')}</span>
          <input
            type="range"
            min={2}
            max={9}
            value={gens}
            onChange={(e) => setGens(+e.target.value)}
            aria-valuetext={`${gens}`}
          />
          <span className="ml-1">{gens}</span>
        </label>
        <label className="text-sm">
          <span className="label">{t('virtual.theme')}</span>
          <select
            className="input !w-auto"
            value={themeId}
            onChange={(e) => setThemeId(e.target.value as Theme['id'])}
            data-testid="v3-theme"
          >
            {Object.keys(THEMES).map((k) => (
              <option key={k} value={k}>
                {t(`virtual.themes.${k}`)}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={spin} onChange={(e) => setSpin(e.target.checked)} />
          {t('virtual.autoRotate')}
        </label>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={showLabels} onChange={(e) => setShowLabels(e.target.checked)} />
          {t('virtual.labels')}
        </label>
        <div className="ml-auto flex gap-2">
          <button type="button" className="btn" onClick={resetCamera}>
            {t('virtual.reset')}
          </button>
          <button type="button" className="btn" onClick={shot}>
            {t('map.screenshot')}
          </button>
        </div>
      </div>
      {!webgl && (
        <p role="alert" className="mb-2 text-sm text-[var(--danger)]">
          {t('virtual.noWebgl')}
        </p>
      )}
      <div className="relative min-h-[360px] flex-1 overflow-hidden rounded-lg border border-[var(--border)]">
        <div
          ref={host}
          className="absolute inset-0"
          data-testid="virtual-canvas"
          data-ready={ready > 1 ? '1' : '0'}
          data-nodes={data?.layout.nodes.length ?? 0}
          data-theme={themeId}
          data-focus={focus ?? ''}
          data-labels={labelCount}
          onPointerMove={(e) => {
            const i = pick(e)
            const p = i >= 0 ? people[i] : undefined
            const r = host.current!.getBoundingClientRect()
            setHover(p ? { x: e.clientX - r.left, y: e.clientY - r.top, p } : null)
          }}
          onPointerLeave={() => setHover(null)}
          onClick={(e) => {
            const i = pick(e)
            if (i >= 0 && people[i]) {
              select(people[i].id)
              setFocus(people[i].id)
            }
          }}
          onDoubleClick={(e) => {
            const i = pick(e)
            if (i >= 0 && people[i]) go('persons', people[i].id)
          }}
        />
        {hover && (
          <div
            className="pointer-events-none absolute z-10 rounded bg-[var(--surface)] px-2 py-1 text-sm shadow"
            style={{ left: hover.x + 12, top: hover.y + 12 }}
            role="tooltip"
          >
            <strong>{hover.p.name || '?'}</strong> <span className="text-[var(--muted)]">{hover.p.life}</span>
          </div>
        )}
        <p
          className="pointer-events-none absolute bottom-2 left-3 text-xs opacity-80"
          style={{ color: theme.label === '#14301a' ? '#14301a' : '#fff' }}
        >
          {t('virtual.hint')}
        </p>
      </div>
      <details className="mt-3 text-sm">
        <summary className="cursor-pointer">{t('virtual.listToggle', { count: uniq.length })}</summary>
        <ul className="mt-2 grid max-h-56 gap-1 overflow-auto sm:grid-cols-2 lg:grid-cols-3" data-testid="virtual-list">
          {data?.layout.nodes.map((n, i) =>
            n.dup_of != null ? null : (
              <li key={`${n.person_id}-${i}`}>
                <button
                  type="button"
                  className="w-full truncate rounded px-2 py-1 text-left hover:bg-[var(--surface-2)]"
                  onClick={() => {
                    flyTo(i)
                    setFocus(n.person_id)
                  }}
                  onDoubleClick={() => select(n.person_id)}
                >
                  {data.people[n.person_id]?.name || '?'}{' '}
                  <span className="text-[var(--muted)]">{data.people[n.person_id]?.life}</span>
                </button>
              </li>
            ),
          )}
        </ul>
      </details>
    </div>
  )
}

function jitterOf(s: string): number {
  let h = 7
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619)
  return ((h >>> 0) % 2001) / 1000 - 1
}
