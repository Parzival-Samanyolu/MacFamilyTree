import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { Summary } from '../api/types'
import { PersonChip } from '../components/PersonChip'
import { act, q } from '../lib/query'
import { useApp } from '../store/app'

export interface Criteria {
  given: string
  surname: string
  phonetic: boolean
  sex: string
  born_from: number | null
  born_to: number | null
  died_from: number | null
  died_to: number | null
  place: string
  event_kind: string
  event_text: string
  status: string
  has_media: boolean | null
  has_sources: boolean | null
  has_parents: boolean | null
  has_children: boolean | null
  bookmarked: boolean | null
}

export const EMPTY: Criteria = {
  given: '',
  surname: '',
  phonetic: false,
  sex: '',
  born_from: null,
  born_to: null,
  died_from: null,
  died_to: null,
  place: '',
  event_kind: '',
  event_text: '',
  status: '',
  has_media: null,
  has_sources: null,
  has_parents: null,
  has_children: null,
  bookmarked: null,
}

const EVENT_KINDS = ['BIRT', 'DEAT', 'MARR', 'BURI', 'CHR', 'RESI', 'OCCU', 'EDUC', 'IMMI', 'EMIG', 'CENS', 'EVEN']
const FLAGS = ['has_media', 'has_sources', 'has_parents', 'has_children', 'bookmarked'] as const

/** Number of criteria that restrict the result (for the "N filters" label). */
export function activeCount(c: Criteria): number {
  return (Object.keys(EMPTY) as (keyof Criteria)[]).filter((k) => {
    if (k === 'phonetic') return false
    const v = c[k]
    return v !== EMPTY[k] && v !== ''
  }).length
}

function numberOrNull(v: string): number | null {
  const n = parseInt(v, 10)
  return Number.isFinite(n) ? n : null
}

interface Result {
  total: number
  items: Summary[]
}
interface Saved {
  name: string
  criteria: Criteria
}

export function AdvancedSearch() {
  const { t } = useTranslation()
  const { select, go, toast } = useApp()
  const [c, setC] = useState<Criteria>(EMPTY)
  const [applied, setApplied] = useState<Criteria | null>(null)
  const [name, setName] = useState('')
  const { data: saved = [] } = useQuery(q<Saved[]>('query.saved'))
  const { data, isFetching } = useQuery({
    ...q<Result>('query.run', { criteria: applied ?? EMPTY, limit: 300 }),
    enabled: !!applied,
  })
  const set = <K extends keyof Criteria>(k: K, v: Criteria[K]) => setC((x) => ({ ...x, [k]: v }))
  const tri = (k: (typeof FLAGS)[number]) => (
    <div key={k}>
      <label className="label" htmlFor={`as-${k}`}>
        {t(`advsearch.flags.${k}`)}
      </label>
      <select
        id={`as-${k}`}
        className="input"
        value={c[k] === null ? '' : String(c[k])}
        onChange={(e) => set(k, e.target.value === '' ? null : e.target.value === 'true')}
      >
        <option value="">{t('advsearch.any')}</option>
        <option value="true">{t('advsearch.yes')}</option>
        <option value="false">{t('advsearch.no')}</option>
      </select>
    </div>
  )
  const text = (k: 'given' | 'surname' | 'place' | 'event_text', label: string) => (
    <div>
      <label className="label" htmlFor={`as-${k}`}>
        {label}
      </label>
      <input id={`as-${k}`} className="input" value={c[k]} onChange={(e) => set(k, e.target.value)} />
    </div>
  )
  const year = (k: 'born_from' | 'born_to' | 'died_from' | 'died_to', label: string) => (
    <div>
      <label className="label" htmlFor={`as-${k}`}>
        {label}
      </label>
      <input
        id={`as-${k}`}
        className="input"
        inputMode="numeric"
        value={c[k] ?? ''}
        onChange={(e) => set(k, numberOrNull(e.target.value))}
      />
    </div>
  )
  const open = (p: Summary) => {
    select(p.id)
    go('persons', p.id)
  }
  const doSave = async () => {
    if (!name.trim()) return
    await act('query.save', { name: name.trim(), criteria: c })
    toast(t('advsearch.saved', { name: name.trim() }))
    setName('')
  }
  return (
    <div className="mx-auto max-w-5xl p-6">
      <h1 className="mb-4 text-2xl font-semibold tracking-tight">{t('nav.advsearch')}</h1>
      <div className="grid gap-4 lg:grid-cols-[1fr_16rem]">
        <form
          className="card grid gap-3 p-4 md:grid-cols-3"
          onSubmit={(e) => {
            e.preventDefault()
            setApplied(c)
          }}
        >
          {text('given', t('person.given'))}
          {text('surname', t('person.surname'))}
          <label className="flex items-end gap-2 pb-2 text-sm">
            <input type="checkbox" checked={c.phonetic} onChange={(e) => set('phonetic', e.target.checked)} />
            {t('advsearch.phonetic')}
          </label>
          {year('born_from', t('advsearch.bornFrom'))}
          {year('born_to', t('advsearch.bornTo'))}
          <div>
            <label className="label" htmlFor="as-sex">
              {t('person.sex')}
            </label>
            <select id="as-sex" className="input" value={c.sex} onChange={(e) => set('sex', e.target.value)}>
              <option value="">{t('advsearch.any')}</option>
              <option value="M">{t('sex.M')}</option>
              <option value="F">{t('sex.F')}</option>
              <option value="U">{t('sex.U')}</option>
            </select>
          </div>
          {year('died_from', t('advsearch.diedFrom'))}
          {year('died_to', t('advsearch.diedTo'))}
          <div>
            <label className="label" htmlFor="as-status">
              {t('advsearch.status')}
            </label>
            <select id="as-status" className="input" value={c.status} onChange={(e) => set('status', e.target.value)}>
              <option value="">{t('advsearch.any')}</option>
              <option value="living">{t('advsearch.living')}</option>
              <option value="deceased">{t('advsearch.deceased')}</option>
            </select>
          </div>
          {text('place', t('advsearch.place'))}
          <div>
            <label className="label" htmlFor="as-kind">
              {t('advsearch.eventKind')}
            </label>
            <select
              id="as-kind"
              className="input"
              value={c.event_kind}
              onChange={(e) => set('event_kind', e.target.value)}
            >
              <option value="">{t('advsearch.any')}</option>
              {EVENT_KINDS.map((k) => (
                <option key={k} value={k}>
                  {t(`event.kind.${k}`, { defaultValue: k })}
                </option>
              ))}
            </select>
          </div>
          {text('event_text', t('advsearch.eventText'))}
          {FLAGS.map(tri)}
          <div className="flex flex-wrap items-center gap-2 md:col-span-3">
            <button type="submit" className="btn btn-primary">
              {t('advsearch.search')}
            </button>
            <button type="button" className="btn" onClick={() => (setC(EMPTY), setApplied(null))}>
              {t('advsearch.clear')}
            </button>
            <span className="flex-1" />
            <label className="sr-only" htmlFor="as-name">
              {t('advsearch.saveAs')}
            </label>
            <input
              id="as-name"
              className="input !w-44"
              placeholder={t('advsearch.saveAs')}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <button type="button" className="btn" disabled={!name.trim()} onClick={() => void doSave()}>
              {t('advsearch.save')}
            </button>
          </div>
        </form>
        <aside aria-label={t('advsearch.savedSearches')}>
          <h2 className="mb-2 font-semibold">{t('advsearch.savedSearches')}</h2>
          <ul className="flex flex-col gap-1" data-testid="saved-searches">
            {saved.length === 0 && <li className="text-sm text-[var(--muted)]">{t('advsearch.noSaved')}</li>}
            {saved.map((s) => (
              <li key={s.name} className="flex items-center gap-1">
                <button
                  type="button"
                  className="flex-1 truncate rounded px-2 py-1 text-left text-sm hover:bg-[var(--surface-2)]"
                  onClick={() => (setC({ ...EMPTY, ...s.criteria }), setApplied({ ...EMPTY, ...s.criteria }))}
                >
                  {s.name}
                </button>
                <button
                  type="button"
                  className="btn btn-danger"
                  aria-label={t('advsearch.deleteSaved', { name: s.name })}
                  onClick={() => void act('query.delete', { name: s.name })}
                >
                  ✕
                </button>
              </li>
            ))}
          </ul>
        </aside>
      </div>
      {applied && (
        <section className="mt-5" aria-live="polite" data-testid="adv-results">
          <h2 className="mb-2 font-semibold">
            {isFetching ? '…' : t('advsearch.results', { count: data?.total ?? 0 })}{' '}
            <span className="text-sm font-normal text-[var(--muted)]">
              {t('advsearch.filters', { count: activeCount(applied) })}
            </span>
          </h2>
          <ul className="grid gap-2 sm:grid-cols-2">
            {(data?.items ?? []).map((p) => (
              <li key={p.id}>
                <PersonChip s={p} onClick={() => open(p)} />
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  )
}
