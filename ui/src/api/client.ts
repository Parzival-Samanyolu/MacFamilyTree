// One command interface, two transports: Tauri `invoke` (desktop) and HTTP (dev server / web build / E2E).

export class ApiError extends Error {
  code: string
  constructor(code: string, message: string) {
    super(message)
    this.code = code
  }
}

type TauriWindow = Window & { __TAURI_INTERNALS__?: unknown }

export async function call<T = unknown>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if ((window as TauriWindow).__TAURI_INTERNALS__) {
    const { invoke } = await import('@tauri-apps/api/core')
    try {
      return (await invoke('call', { cmd, args })) as T
    } catch (e) {
      const err = e as { code?: string; message?: string } | string
      if (typeof err === 'string') throw new ApiError('error', err)
      throw new ApiError(err.code ?? 'error', err.message ?? String(e))
    }
  }
  const res = await fetch(`/api/${cmd}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(args),
  })
  const text = await res.text()
  let body: unknown
  try {
    body = JSON.parse(text)
  } catch {
    throw new ApiError('transport', `Unexpected response (${res.status})`)
  }
  if (!res.ok) {
    const e = body as { code?: string; message?: string }
    throw new ApiError(e.code ?? 'error', e.message ?? 'Request failed')
  }
  return body as T
}

export function bytesToBase64(bytes: Uint8Array): string {
  let s = ''
  const chunk = 0x8000
  for (let i = 0; i < bytes.length; i += chunk) s += String.fromCharCode(...bytes.subarray(i, i + chunk))
  return btoa(s)
}

export function base64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64)
  const out = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i)
  return out
}
