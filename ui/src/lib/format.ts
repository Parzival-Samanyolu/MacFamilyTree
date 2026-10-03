import type { Summary } from '../api/types'

export function initials(s: Pick<Summary, 'given' | 'surname'>): string {
  return `${[...(s.given || '?')][0] ?? ''}${[...(s.surname || '')][0] ?? ''}`.toUpperCase()
}

/** Fixed colours (≥ 4.5:1 against white text) so avatars stay legible in every theme. */
export const SEX_COLORS = { M: '#3a6a96', F: '#a4476e', other: '#66703f' } as const

export function sexColor(sex: string): string {
  return sex === 'M' ? SEX_COLORS.M : sex === 'F' ? SEX_COLORS.F : SEX_COLORS.other
}

export function displayName(s: Pick<Summary, 'name'> | undefined | null, fallback: string): string {
  return s && s.name.trim() ? s.name : fallback
}

export function downloadBlob(name: string, data: BlobPart, type: string) {
  const url = URL.createObjectURL(new Blob([data], { type }))
  const a = document.createElement('a')
  a.href = url
  a.download = name
  document.body.appendChild(a)
  a.click()
  a.remove()
  setTimeout(() => URL.revokeObjectURL(url), 1000)
}
