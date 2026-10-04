import { useQuery } from '@tanstack/react-query'
import { call } from '../api/client'
import { act } from './query'
import { useApp } from '../store/app'

export interface MediaItem {
  id: string
  name: string
  kind: 'image' | 'audio' | 'video' | 'document' | 'other'
  caption: string | null
  hash: string
  width: number | null
  height: number | null
  mode: string
  date: string | null
  size: number | null
  mime: string | null
  exif: { date?: string; lat?: number; lon?: number; camera?: string } | null
  links: number
  has_file: boolean
  has_thumb: boolean
}

export function fileToB64(f: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader()
    r.onerror = () => reject(r.error)
    r.onload = () => resolve(String(r.result).split(',', 2)[1] ?? '')
    r.readAsDataURL(f)
  })
}

export function b64ToBlob(b64: string, mime: string): Blob {
  const bin = atob(b64)
  const bytes = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
  return new Blob([bytes], { type: mime })
}

export function formatSize(n: number | null | undefined): string {
  if (n == null) return ''
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / 1024 / 1024).toFixed(1)} MB`
}

/** Thumbnail as a data URL; undefined while loading or when the item has none. */
export function useThumbUrl(id: string, enabled = true): string | undefined {
  const { data } = useQuery({
    queryKey: ['media.thumb', id],
    enabled,
    staleTime: Infinity,
    queryFn: async () => {
      const r = await call<{ mime: string; data: string } | null>('media.thumb', { id })
      return r ? `data:${r.mime};base64,${r.data}` : null
    },
  })
  return data ?? undefined
}

/** Full-size file as an object URL (images, audio, video, PDF). */
export function useFileUrl(id: string, enabled = true): { url?: string; name?: string } {
  const { data } = useQuery({
    queryKey: ['media.file', id],
    enabled,
    staleTime: Infinity,
    gcTime: 60_000,
    queryFn: async () => {
      const r = await call<{ name: string; mime: string; data: string }>('media.file', { id })
      return { url: URL.createObjectURL(b64ToBlob(r.data, r.mime)), name: r.name }
    },
  })
  return data ?? {}
}

export interface UploadResult {
  added: number
  duplicates: number
  failed: number
}

export async function uploadFiles(
  files: File[],
  target?: { type: string; id: string },
  t?: (k: string, o?: Record<string, unknown>) => string,
): Promise<UploadResult> {
  const res: UploadResult = { added: 0, duplicates: 0, failed: 0 }
  for (const f of files) {
    try {
      const r = await call<{ duplicate: boolean }>('media.import', {
        name: f.name,
        data: await fileToB64(f),
        target_type: target?.type,
        target_id: target?.id,
      })
      if (r.duplicate) res.duplicates++
      else res.added++
    } catch (e) {
      res.failed++
      useApp.getState().toast(`${f.name}: ${e instanceof Error ? e.message : String(e)}`, 'error')
    }
  }
  await act('project.status')
  if (t) useApp.getState().toast(t('media.uploaded', { ...res }), 'info')
  return res
}
