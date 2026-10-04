import { useQuery } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { call } from '../api/client'
import type { Summary } from '../api/types'
import { PersonPicker } from '../components/PersonPicker'
import { downloadBlob } from '../lib/format'
import type { MediaItem } from '../lib/media'
import { act, q } from '../lib/query'

export type Block =
  | { type: 'heading'; text: string }
  | { type: 'text'; text: string }
  | { type: 'quote'; text: string; by?: string | null }
  | { type: 'person'; id: string }
  | { type: 'media'; id: string; caption?: string | null }
  | { type: 'timeline'; id: string }

interface StoryRow {
  id: string
  title: string
  blocks: number
}
interface StoryDoc {
  id: string
  title: string
  blocks: Block[]
}

export const BLOCK_TYPES: Block['type'][] = ['heading', 'text', 'quote', 'person', 'media', 'timeline']

export function emptyBlock(type: Block['type']): Block {
  switch (type) {
    case 'heading':
    case 'text':
      return { type, text: '' }
    case 'quote':
      return { type, text: '', by: '' }
    case 'media':
      return { type, id: '', caption: '' }
    default:
      return { type, id: '' }
  }
}

export function moveBlock<T>(list: T[], from: number, to: number): T[] {
  if (to < 0 || to >= list.length || from === to) return list
  const next = list.slice()
  const [x] = next.splice(from, 1)
  next.splice(to, 0, x)
  return next
}

export function Stories() {
  const { t } = useTranslation()
  const { data: stories = [] } = useQuery(q<StoryRow[]>('story.list'))
  const [sel, setSel] = useState<string | null>(null)
  const current = sel ?? stories[0]?.id ?? null
  const create = async () => {
    const r = await act<{ id: string }>('story.save', { title: t('stories.untitled'), blocks: [] })
    if (r) setSel(r.id)
  }
  return (
    <div className="mx-auto grid max-w-6xl gap-4 p-6 lg:grid-cols-[16rem_1fr]">
      <aside>
        <h1 className="mb-3 text-2xl font-semibold tracking-tight">{t('nav.stories')}</h1>
        <button type="button" className="btn btn-primary mb-3 w-full" onClick={() => void create()}>
          {t('stories.new')}
        </button>
        <ul data-testid="story-list" className="flex flex-col gap-1">
          {stories.length === 0 && <li className="text-sm text-[var(--muted)]">{t('stories.none')}</li>}
          {stories.map((s) => (
            <li key={s.id}>
              <button
                type="button"
                className={`w-full rounded px-2 py-1.5 text-left text-sm hover:bg-[var(--surface-2)] ${s.id === current ? 'bg-[var(--surface-2)] font-medium' : ''}`}
                aria-current={s.id === current}
                onClick={() => setSel(s.id)}
              >
                {s.title} <span className="text-xs text-[var(--muted)]">({s.blocks})</span>
              </button>
            </li>
          ))}
        </ul>
      </aside>
      {current ? (
        <Editor key={current} id={current} onGone={() => setSel(null)} />
      ) : (
        <p className="text-[var(--muted)]">{t('stories.pick')}</p>
      )}
    </div>
  )
}

function Editor({ id, onGone }: { id: string; onGone: () => void }) {
  const { t } = useTranslation()
  const { data } = useQuery({ ...q<StoryDoc>('story.get', { id }), staleTime: Infinity })
  if (!data) return <p aria-busy="true">…</p>
  return <Form key={data.id} doc={data} onGone={onGone} t={t} />
}

function Form({
  doc,
  onGone,
  t,
}: {
  doc: StoryDoc
  onGone: () => void
  t: (k: string, o?: Record<string, unknown>) => string
}) {
  const [title, setTitle] = useState(doc.title)
  const [blocks, setBlocks] = useState<Block[]>(doc.blocks)
  const [lang, setLang] = useState<'en' | 'tr'>('en')
  const [privacy, setPrivacy] = useState<'off' | 'mask' | 'exclude'>('off')
  const [html, setHtml] = useState<{ html: string; markdown: string } | null>(null)
  const [add, setAdd] = useState<Block['type']>('text')
  const [saved, setSaved] = useState(true)
  const lastSaved = useRef(JSON.stringify({ title: doc.title, blocks: doc.blocks }))

  // debounced autosave (only when something changed) + preview refresh
  useEffect(() => {
    const snap = JSON.stringify({ title, blocks })
    if (snap !== lastSaved.current) queueMicrotask(() => setSaved(false))
    let live = true
    const h = setTimeout(async () => {
      if (!title.trim()) return
      try {
        if (snap !== lastSaved.current) {
          await act('story.save', { id: doc.id, title, blocks })
          lastSaved.current = snap
          if (live) setSaved(true)
        }
        const r = await call<{ html: string; markdown: string }>('story.render', { id: doc.id, lang, privacy })
        if (live) setHtml(r)
      } catch {
        /* the preview just keeps its previous content */
      }
    }, 500)
    return () => {
      live = false
      clearTimeout(h)
    }
  }, [title, blocks, lang, privacy, doc.id])

  const patch = (i: number, b: Block) => setBlocks((bs) => bs.map((x, j) => (j === i ? b : x)))
  const slug =
    title
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '') || 'story'
  return (
    <section aria-label={t('stories.editor')} data-testid="story-editor">
      <div className="mb-3 flex flex-wrap items-end gap-3">
        <div className="min-w-60 flex-1">
          <label className="label" htmlFor="story-title">
            {t('stories.title')}
          </label>
          <input id="story-title" className="input" value={title} onChange={(e) => setTitle(e.target.value)} />
        </div>
        <span className="text-xs text-[var(--muted)]" role="status">
          {saved ? t('stories.saved') : t('stories.saving')}
        </span>
        <button
          type="button"
          className="btn btn-danger"
          onClick={async () => {
            if (!window.confirm(t('stories.confirmDelete', { title }))) return
            await act('story.delete', { id: doc.id })
            onGone()
          }}
        >
          {t('common.delete')}
        </button>
      </div>
      <ol className="mb-3 flex flex-col gap-3" data-testid="story-blocks">
        {blocks.map((b, i) => (
          <li key={i} className="card p-3" data-block={b.type}>
            <div className="mb-2 flex items-center gap-2">
              <span className="chip">{t(`stories.block.${b.type}`)}</span>
              <span className="flex-1" />
              <button
                type="button"
                className="btn"
                aria-label={t('stories.up')}
                disabled={i === 0}
                onClick={() => setBlocks(moveBlock(blocks, i, i - 1))}
              >
                ↑
              </button>
              <button
                type="button"
                className="btn"
                aria-label={t('stories.down')}
                disabled={i === blocks.length - 1}
                onClick={() => setBlocks(moveBlock(blocks, i, i + 1))}
              >
                ↓
              </button>
              <button
                type="button"
                className="btn btn-danger"
                aria-label={t('stories.remove')}
                onClick={() => setBlocks(blocks.filter((_, j) => j !== i))}
              >
                ✕
              </button>
            </div>
            <BlockEditor b={b} onChange={(nb) => patch(i, nb)} t={t} />
          </li>
        ))}
      </ol>
      <div className="mb-4 flex items-center gap-2">
        <label className="sr-only" htmlFor="add-block">
          {t('stories.blockType')}
        </label>
        <select
          id="add-block"
          className="input !w-auto"
          value={add}
          onChange={(e) => setAdd(e.target.value as Block['type'])}
        >
          {BLOCK_TYPES.map((k) => (
            <option key={k} value={k}>
              {t(`stories.block.${k}`)}
            </option>
          ))}
        </select>
        <button type="button" className="btn" onClick={() => setBlocks([...blocks, emptyBlock(add)])}>
          {t('stories.addBlock')}
        </button>
      </div>
      <div className="mb-2 flex flex-wrap items-center gap-3">
        <h2 className="font-semibold">{t('stories.preview')}</h2>
        <select
          className="input !w-auto"
          value={lang}
          onChange={(e) => setLang(e.target.value as 'en' | 'tr')}
          aria-label={t('reports.language')}
        >
          <option value="en">English</option>
          <option value="tr">Türkçe</option>
        </select>
        <select
          className="input !w-auto"
          value={privacy}
          onChange={(e) => setPrivacy(e.target.value as typeof privacy)}
          aria-label={t('reports.privacy')}
        >
          <option value="off">{t('reports.privacyOff')}</option>
          <option value="mask">{t('export.livingMask')}</option>
          <option value="exclude">{t('export.livingExclude')}</option>
        </select>
        <span className="flex-1" />
        <button
          type="button"
          className="btn"
          disabled={!html}
          onClick={() => html && downloadBlob(`${slug}.html`, html.html, 'text/html')}
        >
          HTML
        </button>
        <button
          type="button"
          className="btn"
          disabled={!html}
          onClick={() => html && downloadBlob(`${slug}.md`, html.markdown, 'text/markdown')}
        >
          Markdown
        </button>
      </div>
      {/* sandboxed: story HTML is generated from user data and must never run scripts */}
      <iframe
        title={t('stories.preview')}
        srcDoc={html?.html ?? ''}
        sandbox="allow-same-origin"
        className="h-[28rem] w-full rounded-lg border border-[var(--border)] bg-white"
        data-testid="story-frame"
      />
    </section>
  )
}

function PersonBlock({ id, onPick, label }: { id: string; onPick: (id: string) => void; label: string }) {
  const { data } = useQuery({ ...q<{ summary: Summary }>('person.get', { id }), enabled: !!id })
  return (
    <PersonPicker value={id ? (data?.summary ?? null) : null} onChange={(s) => onPick(s?.id ?? '')} label={label} />
  )
}

function BlockEditor({ b, onChange, t }: { b: Block; onChange: (b: Block) => void; t: (k: string) => string }) {
  const { data: media = [] } = useQuery({
    ...q<MediaItem[]>('media.list', { kind: 'image' }),
    enabled: b.type === 'media',
  })
  switch (b.type) {
    case 'heading':
      return (
        <input
          className="input"
          aria-label={t('stories.block.heading')}
          value={b.text}
          onChange={(e) => onChange({ ...b, text: e.target.value })}
        />
      )
    case 'text':
      return (
        <textarea
          className="input min-h-24"
          aria-label={t('stories.block.text')}
          value={b.text}
          onChange={(e) => onChange({ ...b, text: e.target.value })}
        />
      )
    case 'quote':
      return (
        <div className="grid gap-2">
          <textarea
            className="input min-h-16"
            aria-label={t('stories.block.quote')}
            value={b.text}
            onChange={(e) => onChange({ ...b, text: e.target.value })}
          />
          <input
            className="input"
            aria-label={t('stories.by')}
            placeholder={t('stories.by')}
            value={b.by ?? ''}
            onChange={(e) => onChange({ ...b, by: e.target.value })}
          />
        </div>
      )
    case 'person':
    case 'timeline':
      return <PersonBlock id={b.id} label={t('stories.person')} onPick={(id) => onChange({ ...b, id })} />
    case 'media':
      return (
        <div className="grid gap-2 sm:grid-cols-2">
          <select
            className="input"
            aria-label={t('stories.block.media')}
            value={b.id}
            onChange={(e) => onChange({ ...b, id: e.target.value })}
          >
            <option value="">{t('stories.pickImage')}</option>
            {media.map((m) => (
              <option key={m.id} value={m.id}>
                {m.caption || m.name}
              </option>
            ))}
          </select>
          <input
            className="input"
            aria-label={t('media.caption')}
            placeholder={t('media.caption')}
            value={b.caption ?? ''}
            onChange={(e) => onChange({ ...b, caption: e.target.value })}
          />
        </div>
      )
  }
}
