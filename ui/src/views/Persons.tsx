import { useQueries, useQuery } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { call } from '../api/client'
import type { Summary } from '../api/types'
import { Icons } from '../components/Icons'
import { Avatar } from '../components/PersonChip'
import { PersonEditor } from '../components/PersonEditor'
import { act } from '../lib/query'
import { useApp } from '../store/app'

const ROW_H = 54
const PAGE = 100

interface Page {
  total: number
  offset: number
  items: Summary[]
}

function PersonList({ query, sort, bookmarked }: { query: string; sort: string; bookmarked: boolean }) {
  const { t } = useTranslation()
  const { personId, select } = useApp()
  const scroller = useRef<HTMLDivElement>(null)
  const [range, setRange] = useState({ first: 0, last: 30 })
  const base = { query, sort, bookmarked, limit: PAGE }
  const first = useQuery({
    queryKey: ['person.list', { ...base, offset: 0 }],
    queryFn: () => call<Page>('person.list', { ...base, offset: 0 }),
  })
  const total = first.data?.total ?? 0

  const pageIdx = new Set<number>()
  for (let i = range.first; i <= Math.min(range.last, Math.max(total - 1, 0)); i++) pageIdx.add(Math.floor(i / PAGE))
  const pages = [...pageIdx]
  const results = useQueries({
    queries: pages.map((p) => ({
      queryKey: ['person.list', { ...base, offset: p * PAGE }],
      queryFn: () => call<Page>('person.list', { ...base, offset: p * PAGE }),
    })),
  })
  const byIndex = new Map<number, Summary>()
  results.forEach((r, i) => r.data?.items.forEach((it, j) => byIndex.set(pages[i] * PAGE + j, it)))

  const onScroll = useCallback(() => {
    const el = scroller.current
    if (!el) return
    const f = Math.max(0, Math.floor(el.scrollTop / ROW_H) - 5)
    setRange({ first: f, last: f + Math.ceil(el.clientHeight / ROW_H) + 10 })
  }, [])
  useEffect(() => {
    onScroll()
    scroller.current?.scrollTo({ top: 0 })
  }, [query, sort, bookmarked, onScroll])

  const activeRow = [...byIndex.entries()].find(([, p]) => p.id === personId)?.[0] ?? -1
  const rows = []
  for (let i = range.first; i <= Math.min(range.last, total - 1); i++) {
    const p = byIndex.get(i)
    rows.push(
      <div
        key={i}
        id={`prow-${i}`}
        style={{ position: 'absolute', top: i * ROW_H, height: ROW_H, left: 0, right: 0 }}
        role="option"
        aria-selected={p?.id === personId}
        aria-posinset={i + 1}
        aria-setsize={total}
        aria-label={p ? `${p.name || t('person.unnamed')} ${p.life}` : undefined}
        onClick={() => p && select(p.id)}
        className={`flex cursor-pointer items-center gap-3 px-3 ${p && p.id === personId ? 'bg-[var(--accent-soft)]' : 'hover:bg-[var(--surface-2)]'}`}
      >
        {p ? (
          <>
            <Avatar s={p} size={34} />
            <span className="min-w-0 flex-1">
              <span className="block truncate text-sm font-medium">{p.name || t('person.unnamed')}</span>
              <span className="block truncate text-xs text-[var(--muted)]">{p.life}</span>
            </span>
            {p.bookmarked && (
              <span className="text-[var(--warn)]" aria-hidden="true">
                {Icons.star}
              </span>
            )}
          </>
        ) : (
          <div className="h-3 w-40 animate-pulse rounded bg-[var(--surface-2)]" />
        )}
      </div>,
    )
  }
  return (
    <div
      ref={scroller}
      onScroll={onScroll}
      className="relative min-h-0 flex-1 overflow-auto"
      data-testid="person-list"
      tabIndex={0}
      role="listbox"
      aria-label={t('nav.persons')}
      aria-activedescendant={activeRow >= 0 ? `prow-${activeRow}` : undefined}
      onKeyDown={(e) => {
        if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return
        e.preventDefault()
        const idx = [...byIndex.entries()].find(([, p]) => p.id === personId)?.[0] ?? -1
        const next = Math.min(Math.max(idx + (e.key === 'ArrowDown' ? 1 : -1), 0), total - 1)
        const p = byIndex.get(next)
        if (p) select(p.id)
        const el = scroller.current
        if (el) {
          if (next * ROW_H < el.scrollTop) el.scrollTop = next * ROW_H
          else if ((next + 1) * ROW_H > el.scrollTop + el.clientHeight)
            el.scrollTop = (next + 1) * ROW_H - el.clientHeight
        }
      }}
    >
      <div role="presentation" style={{ height: total * ROW_H, position: 'relative' }}>
        {rows}
      </div>
      {total === 0 && !first.isLoading && <p className="p-4 text-sm text-[var(--muted)]">{t('common.noResults')}</p>}
    </div>
  )
}

export function Persons() {
  const { t } = useTranslation()
  const { personId, select, status } = useApp()
  const [query, setQuery] = useState('')
  const [debounced, setDebounced] = useState('')
  const [sort, setSort] = useState('name')
  const [bookmarked, setBookmarked] = useState(false)
  useEffect(() => {
    const h = setTimeout(() => setDebounced(query), 120)
    return () => clearTimeout(h)
  }, [query])

  const addPerson = async () => {
    const r = await act<{ id: string }>('person.create', { given: '', surname: '', sex: 'U' })
    if (r) select(r.id)
  }
  useEffect(() => {
    const h = () => void addPerson()
    window.addEventListener('kt:new-person', h)
    return () => window.removeEventListener('kt:new-person', h)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <div className="flex h-full min-h-0">
      <section
        className="flex w-80 shrink-0 flex-col border-r border-[var(--border)] bg-[var(--surface)]"
        aria-label={t('nav.persons')}
      >
        <div className="flex flex-col gap-2 border-b border-[var(--border)] p-3">
          <div className="flex gap-2">
            <input
              className="input"
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t('persons.search')}
              aria-label={t('persons.search')}
            />
            <button
              type="button"
              className="btn btn-primary"
              onClick={addPerson}
              aria-label={t('persons.add')}
              title={t('persons.add')}
            >
              {Icons.plus}
            </button>
          </div>
          <div className="flex items-center gap-2">
            <select
              className="input"
              value={sort}
              onChange={(e) => setSort(e.target.value)}
              aria-label={t('persons.sort')}
              disabled={!!debounced}
            >
              <option value="name">{t('persons.sortName')}</option>
              <option value="birth">{t('persons.sortBirth')}</option>
              <option value="recent">{t('persons.sortRecent')}</option>
              <option value="added">{t('persons.sortAdded')}</option>
            </select>
            <label className="flex items-center gap-1 whitespace-nowrap text-sm">
              <input type="checkbox" checked={bookmarked} onChange={(e) => setBookmarked(e.target.checked)} />{' '}
              {t('persons.favorites')}
            </label>
          </div>
          <div className="text-xs text-[var(--muted)]">{t('status.persons', { count: status.persons ?? 0 })}</div>
        </div>
        <PersonList query={debounced} sort={sort} bookmarked={bookmarked} />
      </section>
      <section className="min-w-0 flex-1 overflow-auto">
        {!personId && <h1 className="sr-only">{t('nav.persons')}</h1>}
        {personId ? (
          <PersonEditor id={personId} key={personId} />
        ) : (
          <div className="p-10 text-center text-[var(--muted)]">{t('persons.selectHint')}</div>
        )}
      </section>
    </div>
  )
}
