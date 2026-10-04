// Optional online geocoding (Nominatim). Opt-in only; throttled to one request per second with a persistent cache,
// per the Nominatim usage policy. Offline lookup lives in the Rust core (`geo.offline`).

const CACHE_KEY = 'kintree.nominatim.cache'
const OPT_KEY = 'kintree.nominatim.optin'
let last = 0

export interface LatLon {
  lat: number
  lon: number
}

function readCache(): Record<string, LatLon | null> {
  try {
    return JSON.parse(localStorage.getItem(CACHE_KEY) ?? '{}')
  } catch {
    return {}
  }
}

export function onlineEnabled(): boolean {
  try {
    return localStorage.getItem(OPT_KEY) === '1'
  } catch {
    return false
  }
}
export function setOnlineEnabled(on: boolean) {
  try {
    localStorage.setItem(OPT_KEY, on ? '1' : '0')
  } catch {
    /* storage unavailable: the choice just doesn't persist */
  }
}

export async function nominatim(query: string, fetchImpl: typeof fetch = fetch): Promise<LatLon | null> {
  const key = query.trim().toLowerCase()
  const cache = readCache()
  if (key in cache) return cache[key]
  const wait = last + 1100 - Date.now()
  if (wait > 0) await new Promise((r) => setTimeout(r, wait))
  last = Date.now()
  const res = await fetchImpl(
    `https://nominatim.openstreetmap.org/search?format=jsonv2&limit=1&q=${encodeURIComponent(query)}`,
    { headers: { Accept: 'application/json' } },
  )
  if (!res.ok) throw new Error(`Nominatim ${res.status}`)
  const rows = (await res.json()) as { lat: string; lon: string }[]
  const hit = rows[0] ? { lat: Number(rows[0].lat), lon: Number(rows[0].lon) } : null
  try {
    cache[key] = hit
    localStorage.setItem(CACHE_KEY, JSON.stringify(cache))
  } catch {
    /* cache is best effort */
  }
  return hit
}
