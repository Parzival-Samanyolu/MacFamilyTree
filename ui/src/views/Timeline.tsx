import { useQuery } from '@tanstack/react-query'
import * as Tabs from '@radix-ui/react-tabs'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { q } from '../lib/query'
import { useApp } from '../store/app'

interface Entry {
  sort: number
  date_text: string
  kind: string
  person_id: string | null
  family_id: string | null
  text: string
  history: boolean
}
interface Span {
  person_id: string
  name: string
  start_year: number
  end_year: number
  estimated_end: boolean
}

export function Timeline() {
  const { t } = useTranslation()
  const { personId, select, go } = useApp()
  const [scope, setScope] = useState<'all' | 'person'>(personId ? 'person' : 'all')
  const [overlay, setOverlay] = useState(true)
  const [surname, setSurname] = useState('')
  const sc = scope === 'person' && personId ? { scope: 'person', id: personId } : { scope: 'all' }
  const { data } = useQuery(q<{ total: number; entries: Entry[] }>('timeline.get', { ...sc, overlay, limit: 800 }))
  const { data: spans } = useQuery(q<{ total: number; items: Span[] }>('timeline.lifespans', { surname, limit: 120 }))
  const open = (id: string | null) => {
    if (!id) return
    select(id)
    go('persons', id)
  }
  const items = spans?.items ?? []
  const minY = Math.min(...items.map((s) => s.start_year), 3000)
  const maxY = Math.max(...items.map((s) => s.end_year), 0)
  const W = 900
  const rowH = 22
  const left = 170
  const scale = (y: number) => left + ((y - minY) / Math.max(1, maxY - minY)) * (W - left - 20)
  const ticks: number[] = []
  if (items.length) for (let y = Math.ceil(minY / 50) * 50; y <= maxY; y += maxY - minY > 300 ? 100 : 50) ticks.push(y)
  return (
    <div className="mx-auto max-w-5xl p-6">
      <h1 className="mb-4 text-2xl font-semibold tracking-tight">{t('nav.timeline')}</h1>
      <Tabs.Root defaultValue="events">
        <Tabs.List className="mb-4 flex gap-1 border-b border-[var(--border)]">
          {['events', 'lifespans'].map((k) => (
            <Tabs.Trigger
              key={k}
              value={k}
              className="px-3 py-2 text-sm data-[state=active]:border-b-2 data-[state=active]:border-[var(--accent)] data-[state=active]:font-medium data-[state=active]:text-[var(--accent)]"
            >
              {t(`timeline.tab.${k}`)}
            </Tabs.Trigger>
          ))}
        </Tabs.List>
        <Tabs.Content value="events">
          <div className="mb-3 flex flex-wrap items-center gap-3">
            <select
              className="input !w-auto"
              value={scope}
              onChange={(e) => setScope(e.target.value as 'all' | 'person')}
              aria-label={t('timeline.scope')}
            >
              <option value="all">{t('timeline.scopeAll')}</option>
              <option value="person" disabled={!personId}>
                {t('timeline.scopePerson')}
              </option>
            </select>
            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" checked={overlay} onChange={(e) => setOverlay(e.target.checked)} />{' '}
              {t('timeline.overlay')}
            </label>
            <span className="text-sm text-[var(--muted)]">{t('timeline.count', { count: data?.total ?? 0 })}</span>
          </div>
          <ol className="relative border-l-2 border-[var(--border)] pl-5" data-testid="timeline-list">
            {(data?.entries ?? []).map((e, i) => (
              <li key={i} className="mb-3" data-history={e.history ? '1' : undefined}>
                <span
                  className={`absolute -left-[7px] mt-1.5 h-3 w-3 rounded-full ${e.history ? 'bg-[var(--warn)]' : 'bg-[var(--accent)]'}`}
                  aria-hidden="true"
                />
                <div className="text-xs text-[var(--muted)]">{e.date_text}</div>
                {e.history ? (
                  <div className="text-sm italic text-[var(--warn)]">{e.text}</div>
                ) : (
                  <button type="button" className="text-left text-sm hover:underline" onClick={() => open(e.person_id)}>
                    {e.text}
                  </button>
                )}
              </li>
            ))}
          </ol>
        </Tabs.Content>
        <Tabs.Content value="lifespans">
          <div className="mb-3 flex items-center gap-2">
            <label className="label !mb-0" htmlFor="ls-surname">
              {t('person.surname')}
            </label>
            <input
              id="ls-surname"
              className="input !w-48"
              value={surname}
              onChange={(e) => setSurname(e.target.value)}
              placeholder={t('timeline.allSurnames')}
            />
            <span className="text-sm text-[var(--muted)]">
              {t('timeline.showing', { n: items.length, total: spans?.total ?? 0 })}
            </span>
          </div>
          {items.length === 0 ? (
            <p className="card p-6 text-center text-[var(--muted)]">{t('timeline.noSpans')}</p>
          ) : (
            <div className="card overflow-auto p-3">
              <svg
                viewBox={`0 0 ${W} ${items.length * rowH + 30}`}
                width="100%"
                role="group"
                aria-label={t('timeline.tab.lifespans')}
                data-testid="lifespan-chart"
              >
                {ticks.map((y) => (
                  <g key={y}>
                    <line x1={scale(y)} x2={scale(y)} y1={0} y2={items.length * rowH} stroke="var(--border)" />
                    <text
                      x={scale(y)}
                      y={items.length * rowH + 16}
                      fontSize={10}
                      textAnchor="middle"
                      fill="var(--muted)"
                    >
                      {y}
                    </text>
                  </g>
                ))}
                {items.map((s, i) => (
                  <g
                    key={s.person_id}
                    transform={`translate(0 ${i * rowH})`}
                    style={{ cursor: 'pointer' }}
                    role="button"
                    tabIndex={0}
                    aria-label={`${s.name} ${s.start_year}–${s.end_year}`}
                    onClick={() => open(s.person_id)}
                    onKeyDown={(e) => e.key === 'Enter' && open(s.person_id)}
                  >
                    <text x={left - 8} y={14} fontSize={11} textAnchor="end" fill="var(--text)">
                      {s.name.length > 24 ? s.name.slice(0, 23) + '…' : s.name}
                    </text>
                    <rect
                      x={scale(s.start_year)}
                      y={4}
                      width={Math.max(3, scale(s.end_year) - scale(s.start_year))}
                      height={12}
                      rx={4}
                      fill="var(--accent)"
                      opacity={s.estimated_end ? 0.45 : 0.9}
                    />
                    <title>{`${s.name}: ${s.start_year}–${s.estimated_end ? '≈' : ''}${s.end_year}`}</title>
                  </g>
                ))}
              </svg>
              <p className="mt-2 text-xs text-[var(--muted)]">{t('timeline.estimatedHint')}</p>
            </div>
          )}
        </Tabs.Content>
      </Tabs.Root>
    </div>
  )
}
