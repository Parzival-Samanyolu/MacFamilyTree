import type { MediaItem } from '../lib/media'
import { useThumbUrl } from '../lib/media'

const GLYPH: Record<string, string> = { image: '🖼', audio: '♪', video: '▶', document: '📄', other: '📎' }

export function MediaThumb({
  item,
  size = 120,
  alt,
}: {
  item: Pick<MediaItem, 'id' | 'kind' | 'has_thumb' | 'name' | 'caption'>
  size?: number
  alt?: string
}) {
  const url = useThumbUrl(item.id, item.has_thumb)
  return url ? (
    <img
      src={url}
      alt={alt ?? (item.caption || item.name)}
      className="object-cover"
      style={{ width: size, height: size }}
    />
  ) : (
    <span
      className="flex items-center justify-center bg-[var(--surface-2)] text-3xl"
      style={{ width: size, height: size }}
      role="img"
      aria-label={alt ?? item.name}
    >
      {GLYPH[item.kind] ?? GLYPH.other}
    </span>
  )
}
