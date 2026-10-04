import { beforeEach, describe, expect, it, vi } from 'vitest'
import { nominatim } from './geocode'

describe('nominatim', () => {
  beforeEach(() => localStorage.clear())
  it('caches results so repeated queries do not hit the network', async () => {
    const f = vi.fn(async () => new Response(JSON.stringify([{ lat: '41.0', lon: '29.0' }])))
    expect(await nominatim('Üsküdar, Turkey', f as unknown as typeof fetch)).toEqual({ lat: 41, lon: 29 })
    expect(await nominatim(' üsküdar, turkey ', f as unknown as typeof fetch)).toEqual({ lat: 41, lon: 29 })
    expect(f).toHaveBeenCalledTimes(1)
  })
  it('remembers misses and surfaces HTTP errors', async () => {
    const miss = vi.fn(async () => new Response('[]'))
    expect(await nominatim('Atlantis', miss as unknown as typeof fetch)).toBeNull()
    expect(await nominatim('Atlantis', miss as unknown as typeof fetch)).toBeNull()
    expect(miss).toHaveBeenCalledTimes(1)
    const bad = vi.fn(async () => new Response('x', { status: 429 }))
    await expect(nominatim('Elsewhere', bad as unknown as typeof fetch)).rejects.toThrow('429')
  })
})
