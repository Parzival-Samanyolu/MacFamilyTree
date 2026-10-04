import { useQuery } from '@tanstack/react-query'
import * as maplibregl from 'maplibre-gl'
import type { StyleSpecification } from 'maplibre-gl'
import type { Feature, FeatureCollection, Point } from 'geojson'
import 'maplibre-gl/dist/maplibre-gl.css'
import { FileSource, PMTiles, Protocol } from 'pmtiles'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { call } from '../api/client'
import { downloadBlob } from '../lib/format'
import { nominatim, onlineEnabled, setOnlineEnabled } from '../lib/geocode'
import { act, q } from '../lib/query'
import { useApp } from '../store/app'

interface Pt {
  event_id: string
  person_id: string
  name: string
  kind: string
  year: number | null
  lat: number
  lon: number
  place_id: string
  place: string
  inherited: boolean
}
interface Heat {
  place: string
  lat: number
  lon: number
  count: number
}
interface ArcT {
  person_id: string
  name: string
  from: [number, number]
  to: [number, number]
  from_place: string
  to_place: string
  year_from: number | null
  year_to: number | null
}
interface PlaceRow {
  id: string
  name: string
  full_name: string
  lat: number | null
  lon: number | null
  uses: number
}

const KINDS = ['BIRT', 'DEAT', 'MARR', 'RESI', 'BURI', 'OCCU', 'IMMI', 'EMIG'] as const
const KIND_COLOR: Record<string, string> = {
  BIRT: '#2a7a4b',
  DEAT: '#8a3b3b',
  MARR: '#9a5b00',
  RESI: '#2f5f9e',
  BURI: '#6b4f7a',
  OCCU: '#4d6b6b',
  IMMI: '#00796b',
  EMIG: '#a14a76',
}
export type Mode = 'markers' | 'heat' | 'migration' | 'lineage'
type Basemap = 'offline' | 'osm' | 'pmtiles'

maplibregl.setWorkerUrl(new URL('maplibre/maplibre-gl-worker.mjs', document.baseURI).href)

let protocolInstalled: Protocol | null = null
function pmProtocol(): Protocol {
  if (!protocolInstalled) {
    protocolInstalled = new Protocol()
    maplibregl.addProtocol('pmtiles', protocolInstalled.tile)
  }
  return protocolInstalled
}

function isDark(): boolean {
  const th = document.documentElement.dataset.theme
  return th ? th === 'dark' : window.matchMedia('(prefers-color-scheme: dark)').matches
}

function offlineStyle(): StyleSpecification {
  const dark = isDark()
  return {
    version: 8,
    sources: {
      countries: { type: 'geojson', data: new URL('data/world-countries.geojson', document.baseURI).href },
    },
    layers: [
      { id: 'bg', type: 'background', paint: { 'background-color': dark ? '#16222b' : '#cfe0ec' } },
      { id: 'land', type: 'fill', source: 'countries', paint: { 'fill-color': dark ? '#2a3238' : '#f1eee4' } },
      {
        id: 'borders',
        type: 'line',
        source: 'countries',
        paint: { 'line-color': dark ? '#55616a' : '#a9a79c', 'line-width': 0.7 },
      },
    ],
  }
}

function osmStyle(): StyleSpecification {
  return {
    version: 8,
    sources: {
      osm: {
        type: 'raster',
        tiles: ['https://tile.openstreetmap.org/{z}/{x}/{y}.png'],
        tileSize: 256,
        maxzoom: 19,
        attribution: '© OpenStreetMap contributors',
      },
    },
    layers: [{ id: 'osm', type: 'raster', source: 'osm' }],
  }
}

/** Raster tiles render directly; vector tiles (OpenMapTiles-style layer names) get a plain, label-free style. */
async function pmtilesStyle(file: File): Promise<StyleSpecification> {
  const archive = new PMTiles(new FileSource(file))
  pmProtocol().add(archive)
  const header = await archive.getHeader()
  const url = `pmtiles://${file.name}`
  if (header.tileType !== 1) {
    return {
      version: 8,
      sources: { pm: { type: 'raster', url, tileSize: 256 } },
      layers: [{ id: 'pm', type: 'raster', source: 'pm' }],
    }
  }
  const dark = isDark()
  return {
    version: 8,
    sources: { pm: { type: 'vector', url } },
    layers: [
      { id: 'bg', type: 'background', paint: { 'background-color': dark ? '#2a3238' : '#f1eee4' } },
      {
        id: 'water',
        type: 'fill',
        source: 'pm',
        'source-layer': 'water',
        paint: { 'fill-color': dark ? '#16222b' : '#cfe0ec' },
      },
      {
        id: 'roads',
        type: 'line',
        source: 'pm',
        'source-layer': 'transportation',
        paint: { 'line-color': dark ? '#6b747a' : '#bdb9ab', 'line-width': 0.8 },
      },
      {
        id: 'boundary',
        type: 'line',
        source: 'pm',
        'source-layer': 'boundary',
        paint: { 'line-color': dark ? '#55616a' : '#a9a79c', 'line-width': 1 },
      },
    ],
  }
}

/** Gentle great-circle-like curve so overlapping arcs stay distinguishable. */
export function curve(a: [number, number], b: [number, number], steps = 24): [number, number][] {
  const [la1, lo1] = a
  const [la2, lo2] = b
  const mx = (lo1 + lo2) / 2
  const my = (la1 + la2) / 2
  const dx = lo2 - lo1
  const dy = la2 - la1
  const cx = mx - dy * 0.18
  const cy = my + dx * 0.18
  const out: [number, number][] = []
  for (let i = 0; i <= steps; i++) {
    const t = i / steps
    const u = 1 - t
    out.push([u * u * lo1 + 2 * u * t * cx + t * t * lo2, u * u * la1 + 2 * u * t * cy + t * t * la2])
  }
  return out
}

function fc(features: Feature[]): FeatureCollection {
  return { type: 'FeatureCollection', features }
}

export function MapView() {
  const { t } = useTranslation()
  const { personId, select, go } = useApp()
  const box = useRef<HTMLDivElement>(null)
  const mapRef = useRef<maplibregl.Map | null>(null)
  const pickRef = useRef<string | null>(null)
  const [ready, setReady] = useState(0)
  const [mode, setMode] = useState<Mode>('markers')
  const [kinds, setKinds] = useState<string[]>(['BIRT', 'DEAT', 'MARR', 'RESI'])
  const [basemap, setBasemap] = useState<Basemap>('offline')
  const [file, setFile] = useState<File | null>(null)
  const [styleError, setStyleError] = useState('')
  const [onlyPerson, setOnlyPerson] = useState(false)
  const [hideLiving, setHideLiving] = useState(true)
  const [range, setRange] = useState<[number, number] | null>(null)
  const [playing, setPlaying] = useState(false)
  const [pick, setPick] = useState<string | null>(null)
  const [online, setOnline] = useState(onlineEnabled())
  const [busy, setBusy] = useState('')
  useEffect(() => {
    pickRef.current = pick
  }, [pick])

  const base = { kinds, hide_living: hideLiving }
  const { data: pts = [] } = useQuery(q<Pt[]>('map.points', base))
  const { data: arcs = [] } = useQuery(
    q<ArcT[]>('map.arcs', { hide_living: hideLiving, generations: mode === 'lineage' }),
  )
  const { data: places = [] } = useQuery(q<PlaceRow[]>('place.list', {}))
  const missing = places.filter((p) => p.lat == null && p.uses > 0)

  const [minY, maxY] = useMemo(() => {
    const ys = pts.map((p) => p.year).filter((y): y is number => y != null)
    return ys.length ? [Math.min(...ys), Math.max(...ys)] : [0, 0]
  }, [pts])
  const [lo, hi] = range ?? [minY, maxY]
  const inRange = useCallback((y: number | null) => y == null || range == null || (y >= lo && y <= hi), [range, lo, hi])
  const visible = useMemo(
    () => pts.filter((p) => inRange(p.year) && (!onlyPerson || !personId || p.person_id === personId)),
    [pts, inRange, onlyPerson, personId],
  )
  const visibleArcs = useMemo(
    () =>
      arcs.filter(
        (a) => (a.year_to == null || inRange(a.year_to)) && (!onlyPerson || !personId || a.person_id === personId),
      ),
    [arcs, inRange, onlyPerson, personId],
  )
  const route = useMemo(
    () => (onlyPerson && personId ? visible.filter((p) => p.year != null) : []),
    [onlyPerson, personId, visible],
  )

  // Create the map; recreate when the basemap changes.
  useEffect(() => {
    const el = box.current
    if (!el) return
    let cancelled = false
    let map: maplibregl.Map | null = null
    const start = async () => {
      let style: StyleSpecification = offlineStyle()
      setStyleError('')
      try {
        if (basemap === 'osm') style = osmStyle()
        else if (basemap === 'pmtiles' && file) style = await pmtilesStyle(file)
      } catch (e) {
        setStyleError(String(e))
      }
      if (cancelled) return
      map = new maplibregl.Map({
        container: el,
        style,
        center: [32, 39],
        zoom: 3.2,
        attributionControl: { compact: true },
        canvasContextAttributes: { preserveDrawingBuffer: true },
      })
      mapRef.current = map
      map.addControl(new maplibregl.NavigationControl({ showCompass: false }), 'top-right')
      map.addControl(new maplibregl.ScaleControl({ unit: 'metric' }), 'bottom-left')
      map.on('load', () => {
        if (!map) return
        map.addSource('pts', { type: 'geojson', data: fc([]), cluster: true, clusterRadius: 38, clusterMaxZoom: 9 })
        map.addSource('heat', { type: 'geojson', data: fc([]) })
        map.addSource('arcs', { type: 'geojson', data: fc([]) })
        map.addSource('route', { type: 'geojson', data: fc([]) })
        map.addLayer({
          id: 'heat',
          type: 'heatmap',
          source: 'heat',
          paint: {
            'heatmap-weight': ['interpolate', ['linear'], ['get', 'count'], 1, 0.3, 20, 1],
            'heatmap-radius': 28,
            'heatmap-intensity': 1.2,
          },
        })
        map.addLayer({
          id: 'arcs',
          type: 'line',
          source: 'arcs',
          paint: { 'line-color': '#b45309', 'line-width': 1.6, 'line-opacity': 0.75 },
        })
        map.addLayer({
          id: 'route',
          type: 'line',
          source: 'route',
          paint: { 'line-color': '#1d4ed8', 'line-width': 2.5, 'line-dasharray': [2, 1.5] },
        })
        map.addLayer({
          id: 'clusters',
          type: 'circle',
          source: 'pts',
          filter: ['has', 'point_count'],
          paint: {
            'circle-color': '#475569',
            'circle-radius': ['step', ['get', 'point_count'], 12, 10, 16, 50, 22],
            'circle-stroke-color': '#fff',
            'circle-stroke-width': 2,
          },
        })
        map.addLayer({
          id: 'points',
          type: 'circle',
          source: 'pts',
          filter: ['!', ['has', 'point_count']],
          paint: {
            'circle-color': ['get', 'color'],
            'circle-radius': 7,
            'circle-stroke-color': '#fff',
            'circle-stroke-width': 2,
          },
        })
        map.on('click', 'clusters', (e) => {
          const f = e.features?.[0]
          if (!f || !map) return
          const geo = f.geometry as Point
          map.easeTo({ center: geo.coordinates as [number, number], zoom: map.getZoom() + 2 })
        })
        map.on('click', 'points', (e) => {
          const f = e.features?.[0]
          if (!f || !map) return
          const pr = f.properties as {
            name: string
            person_id: string
            kind: string
            year: number | string
            place: string
          }
          const div = document.createElement('div')
          const b = document.createElement('button')
          b.type = 'button'
          b.className = 'font-semibold underline'
          b.textContent = pr.name || '?'
          b.onclick = () => {
            select(pr.person_id)
            go('persons', pr.person_id)
          }
          const s = document.createElement('div')
          s.textContent = `${pr.kind}${pr.year && pr.year !== 'null' ? ` ${pr.year}` : ''} – ${pr.place}`
          div.append(b, s)
          new maplibregl.Popup({ offset: 10 })
            .setLngLat((f.geometry as Point).coordinates as [number, number])
            .setDOMContent(div)
            .addTo(map)
        })
        map.on('click', (e) => {
          const id = pickRef.current
          if (!id) return
          void act('place.set_coords', {
            id,
            lat: Number(e.lngLat.lat.toFixed(5)),
            lon: Number(e.lngLat.lng.toFixed(5)),
          })
          setPick(null)
        })
        map.on('mouseenter', 'points', () => (map!.getCanvas().style.cursor = 'pointer'))
        map.on('mouseleave', 'points', () => (map!.getCanvas().style.cursor = ''))
        setReady((n) => n + 1)
      })
    }
    void start()
    return () => {
      cancelled = true
      map?.remove()
      mapRef.current = null
    }
  }, [basemap, file, select, go])

  // Push data and layer visibility into the map.
  useEffect(() => {
    const map = mapRef.current
    if (!map || !ready || !map.getSource('pts')) return
    const set = (id: string, data: object) =>
      (map.getSource(id) as maplibregl.GeoJSONSource).setData(data as FeatureCollection)
    set(
      'pts',
      fc(
        visible.map((p) => ({
          type: 'Feature',
          geometry: { type: 'Point', coordinates: [p.lon, p.lat] },
          properties: {
            name: p.name,
            person_id: p.person_id,
            kind: p.kind,
            year: p.year,
            place: p.place,
            color: KIND_COLOR[p.kind] ?? '#334155',
          },
        })),
      ),
    )
    const hv = new Map<string, Heat>()
    for (const p of visible) {
      const h = hv.get(p.place_id) ?? { place: p.place, lat: p.lat, lon: p.lon, count: 0 }
      h.count++
      hv.set(p.place_id, h)
    }
    set(
      'heat',
      fc(
        [...hv.values()].map((h) => ({
          type: 'Feature',
          geometry: { type: 'Point', coordinates: [h.lon, h.lat] },
          properties: { count: h.count },
        })),
      ),
    )
    set(
      'arcs',
      fc(
        visibleArcs.map((a) => ({
          type: 'Feature',
          geometry: { type: 'LineString', coordinates: curve(a.from, a.to) },
          properties: { name: a.name },
        })),
      ),
    )
    set(
      'route',
      fc(
        route.length > 1
          ? [
              {
                type: 'Feature',
                geometry: { type: 'LineString', coordinates: route.map((p) => [p.lon, p.lat]) },
                properties: {},
              },
            ]
          : [],
      ),
    )
    const show = (id: string, on: boolean) => map.setLayoutProperty(id, 'visibility', on ? 'visible' : 'none')
    show('heat', mode === 'heat')
    show('arcs', mode === 'migration' || mode === 'lineage')
    show('clusters', mode === 'markers')
    show('points', mode === 'markers')
    show('route', mode === 'markers')
    map.getCanvas().setAttribute('data-feature-count', String(visible.length))
  }, [ready, visible, visibleArcs, route, mode])

  // Fit to data when the data set changes (not on every slider tick).
  const fitKey = pts.length
  useEffect(() => {
    const map = mapRef.current
    if (!map || !ready || !pts.length) return
    const b = new maplibregl.LngLatBounds()
    pts.forEach((p) => b.extend([p.lon, p.lat]))
    map.fitBounds(b, { padding: 60, maxZoom: 8, duration: 0 })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ready, fitKey])

  // Time-slider playback.
  useEffect(() => {
    if (!playing) return
    const id = setInterval(() => {
      setRange((r) => {
        const cur = r ?? [minY, minY]
        const next = cur[1] + Math.max(1, Math.round((maxY - minY) / 60))
        if (next >= maxY) {
          setPlaying(false)
          return [cur[0], maxY]
        }
        return [cur[0], next]
      })
    }, 120)
    return () => clearInterval(id)
  }, [playing, minY, maxY])

  const toggleKind = (k: string) => setKinds((ks) => (ks.includes(k) ? ks.filter((x) => x !== k) : [...ks, k]))

  const exportAs = async (format: 'geojson' | 'kml') => {
    const r = await call<{ data: string; ext: string }>('map.export', { ...base, format })
    downloadBlob(
      `kintree-map.${r.ext}`,
      Uint8Array.from(atob(r.data), (c) => c.charCodeAt(0)),
      'application/octet-stream',
    )
  }
  const geocodeOffline = async () => {
    const r = await act<{ geocoded: number; unresolved: string[] }>('geo.offline')
    if (r) useApp.getState().toast(t('map.geocoded', { count: r.geocoded, left: r.unresolved.length }), 'info')
  }
  const geocodeOnline = async (list: PlaceRow[]) => {
    let done = 0
    for (const p of list) {
      setBusy(t('map.lookingUp', { name: p.full_name }))
      try {
        const hit = await nominatim(p.full_name)
        if (hit) {
          await act('place.set_coords', { id: p.id, lat: hit.lat, lon: hit.lon, status: 'nominatim' })
          done++
        }
      } catch (e) {
        useApp.getState().toast(String(e), 'error')
        break
      }
    }
    setBusy('')
    useApp.getState().toast(t('map.geocoded', { count: done, left: list.length - done }), 'info')
  }
  const shot = () => {
    const map = mapRef.current
    if (!map) return
    map.once('idle', () => map.getCanvas().toBlob((b) => b && downloadBlob('kintree-map.png', b, 'image/png')))
    map.triggerRepaint()
  }

  return (
    <div className="flex h-full flex-col p-4">
      <div className="mb-3 flex flex-wrap items-end gap-x-4 gap-y-2">
        <h1 className="text-2xl font-semibold tracking-tight">{t('nav.map')}</h1>
        <label className="text-sm">
          <span className="label">{t('map.mode')}</span>
          <select
            className="input !w-auto"
            value={mode}
            onChange={(e) => setMode(e.target.value as Mode)}
            data-testid="map-mode"
          >
            {(['markers', 'heat', 'migration', 'lineage'] as Mode[]).map((m) => (
              <option key={m} value={m}>
                {t(`map.modes.${m}`)}
              </option>
            ))}
          </select>
        </label>
        <label className="text-sm">
          <span className="label">{t('map.basemap')}</span>
          <select
            className="input !w-auto"
            value={basemap}
            onChange={(e) => setBasemap(e.target.value as Basemap)}
            data-testid="map-basemap"
          >
            <option value="offline">{t('map.base.offline')}</option>
            <option value="osm">{t('map.base.osm')}</option>
            <option value="pmtiles">{t('map.base.pmtiles')}</option>
          </select>
        </label>
        {basemap === 'pmtiles' && (
          <label className="text-sm">
            <span className="label">{t('map.pmtilesFile')}</span>
            <input type="file" accept=".pmtiles" onChange={(e) => setFile(e.target.files?.[0] ?? null)} />
          </label>
        )}
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={onlyPerson}
            disabled={!personId}
            onChange={(e) => setOnlyPerson(e.target.checked)}
          />
          {t('map.onlySelected')}
        </label>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={hideLiving} onChange={(e) => setHideLiving(e.target.checked)} />
          {t('map.hideLiving')}
        </label>
        <div className="ml-auto flex gap-2">
          <button type="button" className="btn" onClick={() => void exportAs('geojson')}>
            GeoJSON
          </button>
          <button type="button" className="btn" onClick={() => void exportAs('kml')}>
            KML
          </button>
          <button type="button" className="btn" onClick={shot}>
            {t('map.screenshot')}
          </button>
        </div>
      </div>
      <fieldset className="mb-3 flex flex-wrap gap-3 text-sm">
        <legend className="sr-only">{t('map.kinds')}</legend>
        {KINDS.map((k) => (
          <label key={k} className="flex items-center gap-1.5">
            <input type="checkbox" checked={kinds.includes(k)} onChange={() => toggleKind(k)} />
            <span
              className="inline-block h-2.5 w-2.5 rounded-full"
              style={{ background: KIND_COLOR[k] }}
              aria-hidden="true"
            />
            {t(`event.kind.${k}`, { defaultValue: k })}
          </label>
        ))}
      </fieldset>
      {minY !== maxY && (
        <div className="mb-3 flex flex-wrap items-center gap-3 text-sm" data-testid="map-slider">
          <button type="button" className="btn" onClick={() => (setRange([minY, minY]), setPlaying(true))}>
            {t('map.play')}
          </button>
          <button type="button" className="btn" onClick={() => (setPlaying(false), setRange(null))}>
            {t('map.reset')}
          </button>
          <label className="flex items-center gap-2">
            {t('map.from')} {lo}
            <input
              type="range"
              min={minY}
              max={maxY}
              value={lo}
              aria-label={t('map.from')}
              onChange={(e) => setRange([Math.min(+e.target.value, hi), hi])}
            />
          </label>
          <label className="flex items-center gap-2">
            {t('map.to')} {hi}
            <input
              type="range"
              min={minY}
              max={maxY}
              value={hi}
              aria-label={t('map.to')}
              onChange={(e) => setRange([lo, Math.max(+e.target.value, lo)])}
            />
          </label>
        </div>
      )}
      {styleError && (
        <p role="alert" className="mb-2 text-sm text-[var(--danger)]">
          {styleError}
        </p>
      )}
      {basemap === 'osm' && <p className="mb-2 text-xs text-[var(--muted)]">{t('map.osmNote')}</p>}
      <div className="grid min-h-0 flex-1 gap-3 lg:grid-cols-[1fr_20rem]">
        <div className="relative min-h-[360px] overflow-hidden rounded-lg border border-[var(--border)]">
          <div
            ref={box}
            className="absolute inset-0"
            data-testid="map-canvas"
            data-ready={ready ? '1' : '0'}
            data-visible={visible.length}
            data-arcs={visibleArcs.length}
            role="application"
            aria-label={t('map.aria')}
          />
          {pick && (
            <div className="absolute left-2 top-2 rounded bg-[var(--surface)] px-3 py-1.5 text-sm shadow" role="status">
              {t('map.pickHint')}
            </div>
          )}
        </div>
        <aside className="min-h-0 overflow-auto text-sm" aria-label={t('map.places')}>
          <div className="mb-2 flex flex-wrap gap-2">
            <button
              type="button"
              className="btn"
              onClick={() => void geocodeOffline()}
              data-testid="map-geocode-offline"
            >
              {t('map.geocodeOffline')}
            </button>
          </div>
          <label className="mb-2 flex items-start gap-2 text-xs">
            <input
              type="checkbox"
              checked={online}
              onChange={(e) => (setOnlineEnabled(e.target.checked), setOnline(e.target.checked))}
            />
            <span>{t('map.onlineOptIn')}</span>
          </label>
          {online && missing.length > 0 && (
            <button type="button" className="btn mb-2" disabled={!!busy} onClick={() => void geocodeOnline(missing)}>
              {t('map.geocodeOnline', { count: missing.length })}
            </button>
          )}
          {busy && <p className="mb-2 text-xs text-[var(--muted)]">{busy}</p>}
          <h2 className="mb-1 font-semibold">{t('map.missing', { count: missing.length })}</h2>
          <ul data-testid="map-missing">
            {missing.slice(0, 80).map((p) => (
              <li key={p.id} className="flex items-center justify-between gap-2 border-b border-[var(--border)] py-1">
                <span className="truncate">{p.full_name}</span>
                <button
                  type="button"
                  className="btn !px-2 !py-0.5 text-xs"
                  onClick={() => setPick(pick === p.id ? null : p.id)}
                  aria-pressed={pick === p.id}
                >
                  {t('map.pick')}
                </button>
              </li>
            ))}
          </ul>
        </aside>
      </div>
      <details className="mt-3 text-sm">
        <summary className="cursor-pointer">{t('map.tableToggle', { count: visible.length })}</summary>
        <table className="mt-2 w-full text-left" data-testid="map-table">
          <thead>
            <tr>
              <th scope="col">{t('map.col.name')}</th>
              <th scope="col">{t('map.col.event')}</th>
              <th scope="col">{t('map.col.year')}</th>
              <th scope="col">{t('map.col.place')}</th>
            </tr>
          </thead>
          <tbody>
            {visible.slice(0, 300).map((p) => (
              <tr key={p.event_id}>
                <td>{p.name}</td>
                <td>{t(`event.kind.${p.kind}`, { defaultValue: p.kind })}</td>
                <td>{p.year ?? ''}</td>
                <td>
                  {p.place}
                  {p.inherited ? ` (${t('map.approx')})` : ''}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </div>
  )
}
