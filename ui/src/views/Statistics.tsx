import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { call } from '../api/client'
import type { Bucket, Stats, Summary } from '../api/types'
import { Modal } from '../components/Modal'
import { PersonChip } from '../components/PersonChip'
import { q } from '../lib/query'
import { useApp } from '../store/app'

function Bars({
  title,
  buckets,
  onPick,
  label,
}: {
  title: string
  buckets: Bucket[]
  onPick: (b: Bucket) => void
  label?: (b: Bucket) => string
}) {
  const max = Math.max(1, ...buckets.map((b) => b.count))
  const W = 440
  const H = 150
  const bw = W / buckets.length
  return (
    <section className="card p-4">
      <h2 className="mb-2 text-sm font-semibold uppercase tracking-wide text-[var(--muted)]">{title}</h2>
      <svg viewBox={`0 0 ${W} ${H + 24}`} width="100%" role="group" aria-label={title}>
        {buckets.map((b, i) => {
          const h = (b.count / max) * H
          return (
            <g
              key={b.label}
              transform={`translate(${i * bw} 0)`}
              style={{ cursor: b.count ? 'pointer' : 'default' }}
              onClick={() => b.count && onPick(b)}
              role="button"
              tabIndex={b.count ? 0 : -1}
              aria-label={`${b.label}: ${b.count}`}
              onKeyDown={(e) => e.key === 'Enter' && b.count && onPick(b)}
            >
              <rect
                x={3}
                y={H - h}
                width={bw - 6}
                height={Math.max(h, b.count ? 2 : 0)}
                rx={3}
                fill="var(--accent)"
                opacity={0.85}
              />
              {b.count > 0 && (
                <text x={bw / 2} y={H - h - 3} textAnchor="middle" fontSize={9} fill="var(--text)">
                  {b.count}
                </text>
              )}
              <text x={bw / 2} y={H + 14} textAnchor="middle" fontSize={bw < 28 ? 7 : 9} fill="var(--muted)">
                {label ? label(b) : b.label}
              </text>
            </g>
          )
        })}
      </svg>
    </section>
  )
}

function TopList({ title, rows }: { title: string; rows: [string, number][] }) {
  const max = Math.max(1, ...rows.map((r) => r[1]))
  return (
    <section className="card p-4">
      <h2 className="mb-2 text-sm font-semibold uppercase tracking-wide text-[var(--muted)]">{title}</h2>
      <ul className="flex flex-col gap-1 text-sm">
        {rows.map(([name, n]) => (
          <li key={name} className="flex items-center gap-2">
            <span className="w-32 truncate">{name}</span>
            <span className="h-2 rounded bg-[var(--accent)]" style={{ width: `${(n / max) * 100}px` }} />
            <span className="text-[var(--muted)]">{n}</span>
          </li>
        ))}
        {rows.length === 0 && <li className="text-[var(--muted)]">—</li>}
      </ul>
    </section>
  )
}

export function Statistics() {
  const { t } = useTranslation()
  const { data } = useQuery(q<Stats>('stats.compute'))
  const { select, go } = useApp()
  const [drill, setDrill] = useState<{ title: string; ids: string[] } | null>(null)
  const { data: people } = useQuery({
    queryKey: ['stats.persons', drill?.ids],
    queryFn: () => call<Summary[]>('stats.persons', { ids: drill!.ids }),
    enabled: !!drill,
  })
  if (!data) return <div className="p-6 text-[var(--muted)]">…</div>
  const c = data.counts
  const stat = (k: string, v: string | number) => (
    <div className="card p-3">
      <div className="text-xs text-[var(--muted)]">{t(`stats.${k}`)}</div>
      <div className="text-2xl font-semibold" data-testid={`stat-${k}`}>
        {v}
      </div>
    </div>
  )
  const pick = (title: string) => (b: Bucket) => setDrill({ title: `${title}: ${b.label}`, ids: b.ids })
  return (
    <div className="mx-auto max-w-5xl p-6">
      <h1 className="mb-4 text-2xl font-semibold tracking-tight">{t('nav.statistics')}</h1>
      <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-4 lg:grid-cols-8">
        {stat('persons', c.persons)}
        {stat('families', c.families)}
        {stat('events', c.events)}
        {stat('places', c.places)}
        {stat('sources', c.sources)}
        {stat('media', c.media)}
        {stat('notes', c.notes)}
        {stat('generations', data.generation_depth)}
      </div>
      <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-4">
        {stat('avgCompleteness', `${data.avg_completeness.toFixed(0)}%`)}
        {stat('sourceCoverage', `${data.source_coverage_pct.toFixed(0)}%`)}
        {stat('avgMarriageAge', data.avg_marriage_age === null ? '—' : data.avg_marriage_age.toFixed(1))}
        {stat('longestLived', data.longest_lived[0] ? Math.round(data.longest_lived[0][1]) : '—')}
      </div>
      <div className="grid gap-4 md:grid-cols-2">
        <Bars
          title={t('stats.sex')}
          buckets={data.sex.map((b) => ({ ...b, label: b.label }))}
          label={(b) => t(`sex.${b.label}`)}
          onPick={pick(t('stats.sex'))}
        />
        <Bars title={t('stats.ageAtDeath')} buckets={data.age_at_death} onPick={pick(t('stats.ageAtDeath'))} />
        <Bars
          title={t('stats.childrenPerFamily')}
          buckets={data.children_per_family}
          onPick={pick(t('stats.childrenPerFamily'))}
        />
        <Bars title={t('stats.birthMonths')} buckets={data.birth_months} onPick={pick(t('stats.birthMonths'))} />
        <section className="card p-4">
          <h2 className="mb-2 text-sm font-semibold uppercase tracking-wide text-[var(--muted)]">
            {t('stats.lifespanByCentury')}
          </h2>
          <ul className="text-sm">
            {data.lifespan_by_century.map(([cent, avg, n]) => (
              <li key={cent} className="flex justify-between border-t border-[var(--border)] py-1">
                <span>{cent}s</span>
                <span>
                  {avg.toFixed(1)} <span className="text-[var(--muted)]">(n={n})</span>
                </span>
              </li>
            ))}
            {data.lifespan_by_century.length === 0 && <li className="text-[var(--muted)]">—</li>}
          </ul>
        </section>
        <TopList title={t('stats.topSurnames')} rows={data.top_surnames} />
        <TopList title={t('stats.topGiven')} rows={data.top_given_names} />
        <TopList title={t('stats.topOccupations')} rows={data.top_occupations} />
        <TopList title={t('stats.topPlaces')} rows={data.top_places} />
      </div>
      <Modal open={!!drill} onOpenChange={(o) => !o && setDrill(null)} title={drill?.title ?? ''}>
        <ul className="flex max-h-96 flex-col gap-2 overflow-auto">
          {(people ?? []).map((p) => (
            <li key={p.id}>
              <PersonChip
                s={p}
                onClick={() => {
                  setDrill(null)
                  select(p.id)
                  go('persons', p.id)
                }}
              />
            </li>
          ))}
          {drill && drill.ids.length > 500 && (
            <li className="text-xs text-[var(--muted)]">{t('stats.firstN', { n: 500, total: drill.ids.length })}</li>
          )}
        </ul>
      </Modal>
    </div>
  )
}
