import { useQuery } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { call } from '../api/client'
import type { Summary } from '../api/types'
import { MediaThumb } from '../components/MediaThumb'
import { Modal } from '../components/Modal'
import { PersonPicker } from '../components/PersonPicker'
import { downloadBlob } from '../lib/format'
import { fileToB64, formatSize, uploadFiles, useFileUrl, type MediaItem } from '../lib/media'
import { act, q } from '../lib/query'
import { useApp } from '../store/app'

interface Detail {
  item: MediaItem
  links: { target_type: string; target_id: string; label: string | null }[]
  place: string | null
}
interface Suggestion {
  date: string | null
  place: { id: string; name: string; distance_km: number } | null
  camera: string | null
}

export function MediaView() {
  const { t } = useTranslation()
  const { mediaFocus, setMediaFocus } = useApp()
  const [text, setText] = useState('')
  const [kind, setKind] = useState('')
  const [unlinked, setUnlinked] = useState(false)
  const [missing, setMissing] = useState(false)
  const [open, setOpen] = useState<string | null>(mediaFocus)
  const [show, setShow] = useState<number | null>(null)
  const [over, setOver] = useState(false)
  const fileInput = useRef<HTMLInputElement>(null)
  const { data: items = [] } = useQuery(q<MediaItem[]>('media.list', { q: text, kind, unlinked, missing }))
  useEffect(() => {
    if (mediaFocus) setMediaFocus(null)
  }, [mediaFocus, setMediaFocus])
  const images = items.filter((i) => i.kind === 'image' && i.has_file)
  const upload = async (files: FileList | File[]) => {
    await uploadFiles([...files], undefined, t)
    await act('project.status')
  }
  return (
    <div
      className={`mx-auto max-w-6xl p-6 ${over ? 'outline outline-2 outline-[var(--accent)]' : ''}`}
      onDragOver={(e) => (e.preventDefault(), setOver(true))}
      onDragLeave={() => setOver(false)}
      onDrop={(e) => {
        e.preventDefault()
        setOver(false)
        void upload(e.dataTransfer.files)
      }}
    >
      <h1 className="mb-4 text-2xl font-semibold tracking-tight">{t('nav.media')}</h1>
      <div className="mb-4 flex flex-wrap items-center gap-3">
        <input
          ref={fileInput}
          type="file"
          multiple
          className="sr-only"
          data-testid="media-file-input"
          aria-label={t('media.addFiles')}
          onChange={(e) => e.target.files && void upload(e.target.files).then(() => (e.target.value = ''))}
        />
        <button type="button" className="btn btn-primary" onClick={() => fileInput.current?.click()}>
          {t('media.addFiles')}
        </button>
        <input
          type="search"
          className="input !w-56"
          placeholder={t('media.search')}
          aria-label={t('media.search')}
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
        <select
          className="input !w-auto"
          value={kind}
          onChange={(e) => setKind(e.target.value)}
          aria-label={t('media.kind')}
        >
          <option value="">{t('media.allKinds')}</option>
          {['image', 'document', 'audio', 'video', 'other'].map((k) => (
            <option key={k} value={k}>
              {t(`media.kinds.${k}`)}
            </option>
          ))}
        </select>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={unlinked} onChange={(e) => setUnlinked(e.target.checked)} />
          {t('media.unlinked')}
        </label>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={missing} onChange={(e) => setMissing(e.target.checked)} />
          {t('media.missing')}
        </label>
        <button type="button" className="btn" disabled={!images.length} onClick={() => setShow(0)}>
          {t('media.slideshow')}
        </button>
        <span className="text-sm text-[var(--muted)]">{t('media.count', { count: items.length })}</span>
      </div>
      {items.length === 0 ? (
        <p className="rounded-lg border border-dashed border-[var(--border)] p-10 text-center text-[var(--muted)]">
          {t('media.empty')}
        </p>
      ) : (
        <ul className="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-3" data-testid="media-grid">
          {items.map((m) => (
            <li key={m.id}>
              <button
                type="button"
                className="card flex w-full flex-col items-center gap-1 overflow-hidden p-0 text-left hover:shadow-md"
                onClick={() => setOpen(m.id)}
                data-testid="media-card"
                data-name={m.name}
              >
                <MediaThumb item={m} size={150} />
                <span className="w-full truncate px-2 pb-1 text-xs">
                  {m.caption || m.name}
                  {!m.has_file && <span className="ml-1 text-[var(--warn)]">⚠</span>}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
      {open && <MediaDetail id={open} onClose={() => setOpen(null)} />}
      {show !== null && <Slideshow items={images} start={show} onClose={() => setShow(null)} />}
    </div>
  )
}

export function MediaDetail({ id, onClose }: { id: string; onClose: () => void }) {
  const { t } = useTranslation()
  const { select, go } = useApp()
  const { data } = useQuery(q<Detail>('media.get', { id }))
  const { data: sug } = useQuery(q<Suggestion>('media.suggest', { id }))
  const item = data?.item
  const { url } = useFileUrl(id, !!item?.has_file)
  const [adding, setAdding] = useState<Summary | null>(null)
  const relink = useRef<HTMLInputElement>(null)
  if (!item || !data)
    return (
      <Modal open onOpenChange={(o) => !o && onClose()} title="…" wide>
        <p aria-busy="true">…</p>
      </Modal>
    )
  const personLinks = data.links.filter((l) => l.target_type === 'person')
  const save = (patch: Record<string, unknown>) => act('media.update', { id, ...patch })
  const download = async () => {
    const f = await call<{ name: string; mime: string; data: string }>('media.file', { id })
    const bin = Uint8Array.from(atob(f.data), (c) => c.charCodeAt(0))
    downloadBlob(f.name, bin, f.mime)
  }
  return (
    <Modal open onOpenChange={(o) => !o && onClose()} title={item.caption || item.name} wide>
      <div className="grid gap-4 md:grid-cols-[1fr_16rem]" data-testid="media-detail">
        <div className="flex min-h-40 items-center justify-center rounded-lg bg-[var(--surface-2)] p-2">
          {!item.has_file ? (
            <p className="text-[var(--warn)]">{t('media.noFile')}</p>
          ) : item.kind === 'image' && url ? (
            <img src={url} alt={item.caption || item.name} className="max-h-[50vh] max-w-full object-contain" />
          ) : item.kind === 'audio' && url ? (
            <audio controls src={url} aria-label={item.name} />
          ) : item.kind === 'video' && url ? (
            <video controls src={url} className="max-h-[50vh] max-w-full" aria-label={item.name} />
          ) : url ? (
            <a className="underline" href={url} target="_blank" rel="noreferrer">
              {t('media.open')} {item.name}
            </a>
          ) : (
            <MediaThumb item={item} size={160} />
          )}
        </div>
        <div className="flex flex-col gap-3 text-sm">
          <div className="text-xs text-[var(--muted)]">
            {item.name} · {item.mime} · {formatSize(item.size)}
            {item.width ? ` · ${item.width}×${item.height}` : ''}
          </div>
          <div>
            <label className="label" htmlFor="m-cap">
              {t('media.caption')}
            </label>
            <input
              id="m-cap"
              className="input"
              defaultValue={item.caption ?? ''}
              onBlur={(e) => e.target.value !== (item.caption ?? '') && void save({ caption: e.target.value })}
            />
          </div>
          <div>
            <label className="label" htmlFor="m-date">
              {t('media.date')}
            </label>
            <input
              id="m-date"
              className="input"
              defaultValue={item.date ?? ''}
              onBlur={(e) => e.target.value !== (item.date ?? '') && void save({ date_text: e.target.value })}
            />
          </div>
          <div>
            <label className="label" htmlFor="m-place">
              {t('media.place')}
            </label>
            <input
              id="m-place"
              className="input"
              defaultValue={data.place ?? ''}
              onBlur={(e) => e.target.value !== (data.place ?? '') && void save({ place_text: e.target.value })}
            />
          </div>
          {sug && (sug.date || sug.place) && (
            <div className="rounded-lg border border-[var(--border)] p-2" data-testid="media-suggest">
              <div className="mb-1 font-medium">{t('media.suggestions')}</div>
              {sug.date && (
                <button type="button" className="btn mr-2" onClick={() => void save({ date_text: sug.date })}>
                  {t('media.useDate', { date: sug.date })}
                </button>
              )}
              {sug.place && (
                <button type="button" className="btn" onClick={() => void save({ place_text: sug.place!.name })}>
                  {t('media.usePlace', { place: sug.place.name, km: sug.place.distance_km })}
                </button>
              )}
            </div>
          )}
        </div>
      </div>
      <h2 className="mb-1 mt-4 font-semibold">{t('media.linkedTo')}</h2>
      <ul className="mb-2 flex flex-wrap gap-2" data-testid="media-links">
        {data.links.length === 0 && <li className="text-sm text-[var(--muted)]">{t('media.noLinks')}</li>}
        {data.links.map((l) => (
          <li key={`${l.target_type}-${l.target_id}`} className="chip flex items-center gap-1">
            {l.target_type === 'person' ? (
              <button
                type="button"
                className="underline"
                onClick={() => {
                  select(l.target_id)
                  go('persons', l.target_id)
                  onClose()
                }}
              >
                {l.label}
              </button>
            ) : (
              <span>{l.label ?? l.target_type}</span>
            )}
            {l.target_type === 'person' && (
              <button
                type="button"
                className="text-xs underline"
                onClick={() => void act('media.set_primary', { person_id: l.target_id, media_id: id })}
              >
                {t('media.makeProfile')}
              </button>
            )}
            <button
              type="button"
              aria-label={t('media.unlink')}
              onClick={() => void act('media.unlink', { id, target_type: l.target_type, target_id: l.target_id })}
            >
              ✕
            </button>
          </li>
        ))}
      </ul>
      <div className="flex flex-wrap items-end gap-3">
        <div className="w-64">
          <PersonPicker
            value={adding}
            onChange={setAdding}
            label={t('media.linkPerson')}
            exclude={personLinks.map((l) => l.target_id)}
          />
        </div>
        <button
          type="button"
          className="btn"
          disabled={!adding}
          onClick={async () => {
            if (!adding) return
            await act('media.link', { id, target_type: 'person', target_id: adding.id })
            setAdding(null)
          }}
        >
          {t('media.link')}
        </button>
        <span className="flex-1" />
        <input
          ref={relink}
          type="file"
          className="sr-only"
          aria-label={t('media.relink')}
          onChange={async (e) => {
            const f = e.target.files?.[0]
            if (f) await act('media.relink', { id, name: f.name, data: await fileToB64(f) })
          }}
        />
        <button type="button" className="btn" onClick={() => relink.current?.click()}>
          {t('media.relink')}
        </button>
        {item.has_file && (
          <button type="button" className="btn" onClick={() => void download()}>
            {t('media.download')}
          </button>
        )}
        <button
          type="button"
          className="btn btn-danger"
          onClick={async () => {
            if (!window.confirm(t('media.confirmDelete', { name: item.name }))) return
            await act('media.delete', { id })
            onClose()
            useApp
              .getState()
              .toast(t('toast.deleted'), 'info', { label: t('action.undo'), run: () => void act('history.undo') })
          }}
        >
          {t('common.delete')}
        </button>
      </div>
    </Modal>
  )
}

function Slideshow({ items, start, onClose }: { items: MediaItem[]; start: number; onClose: () => void }) {
  const { t } = useTranslation()
  const [i, setI] = useState(start)
  const [playing, setPlaying] = useState(false)
  const m = items[i]
  const { url } = useFileUrl(m?.id ?? '', !!m)
  const step = useCallback((d: number) => setI((n) => (n + d + items.length) % items.length), [items.length])
  useEffect(() => {
    if (!playing) return
    const h = setInterval(() => step(1), 3000)
    return () => clearInterval(h)
  }, [playing, step])
  return (
    <Modal open onOpenChange={(o) => !o && onClose()} title={t('media.slideshow')} wide>
      <div
        data-testid="slideshow"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === 'ArrowRight') step(1)
          if (e.key === 'ArrowLeft') step(-1)
        }}
      >
        <div className="flex h-[55vh] items-center justify-center rounded-lg bg-black/80">
          {url && m && <img src={url} alt={m.caption || m.name} className="max-h-full max-w-full object-contain" />}
        </div>
        <p className="mt-2 text-center text-sm" aria-live="polite">
          {m?.caption || m?.name}{' '}
          <span className="text-[var(--muted)]">
            ({i + 1}/{items.length})
          </span>
        </p>
        <div className="mt-2 flex justify-center gap-2">
          <button type="button" className="btn" onClick={() => step(-1)}>
            {t('media.prev')}
          </button>
          <button type="button" className="btn" onClick={() => setPlaying((p) => !p)} aria-pressed={playing}>
            {playing ? t('media.pause') : t('media.play')}
          </button>
          <button type="button" className="btn" onClick={() => step(1)}>
            {t('media.next')}
          </button>
        </div>
      </div>
    </Modal>
  )
}
