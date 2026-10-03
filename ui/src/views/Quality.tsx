import * as Tabs from '@radix-ui/react-tabs'
import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { DuplicateCandidate, Finding } from '../api/types'
import { Avatar } from '../components/PersonChip'
import { act, q } from '../lib/query'
import { useApp } from '../store/app'

const SEV_ORDER = { Error: 0, Warning: 1, Info: 2 } as const

function Plausibility() {
  const { t } = useTranslation()
  const { select, go } = useApp()
  const { data, isLoading } = useQuery(q<Finding[]>('quality.check'))
  const [sev, setSev] = useState<'all' | 'Error' | 'Warning' | 'Info'>('all')
  const rows = (data ?? [])
    .filter((f) => sev === 'all' || f.severity === sev)
    .sort((a, b) => SEV_ORDER[a.severity] - SEV_ORDER[b.severity])
  const open = (pid: string) => {
    select(pid)
    go('persons', pid)
  }
  const counts = (s: string) => (data ?? []).filter((f) => f.severity === s).length
  return (
    <div>
      <div className="mb-3 flex flex-wrap items-center gap-2">
        {(['all', 'Error', 'Warning', 'Info'] as const).map((s) => (
          <button
            key={s}
            type="button"
            className={`btn ${sev === s ? 'btn-primary' : ''}`}
            aria-pressed={sev === s}
            onClick={() => setSev(s)}
          >
            {s === 'all' ? t('quality.all') : t(`quality.sev.${s}`)}{' '}
            {s !== 'all' && <span className="text-xs opacity-80">{counts(s)}</span>}
          </button>
        ))}
      </div>
      {isLoading && <p className="text-[var(--muted)]">…</p>}
      {!isLoading && rows.length === 0 && (
        <p className="card p-6 text-center text-[var(--muted)]" data-testid="quality-clean">
          {t('quality.clean')}
        </p>
      )}
      <ul className="flex flex-col gap-2">
        {rows.map((f) => (
          <li key={f.id} className="card flex items-center gap-3 p-3" data-testid="finding" data-rule={f.rule}>
            <span
              className={`chip ${f.severity === 'Error' ? '!bg-[var(--danger)] !text-white' : f.severity === 'Warning' ? '!bg-[var(--warn)] !text-white' : ''}`}
            >
              {t(`quality.sev.${f.severity}`)}
            </span>
            <div className="min-w-0 flex-1">
              <div className="text-sm">{f.message}</div>
              <div className="text-xs text-[var(--muted)]">{t(`quality.rule.${f.rule}`)}</div>
            </div>
            {f.person_id && (
              <button type="button" className="btn" onClick={() => open(f.person_id!)}>
                {t('quality.open')}
              </button>
            )}
            {f.fix && (
              <button type="button" className="btn" onClick={() => act('quality.fix', { fix: f.fix })}>
                {t('quality.fix')}
              </button>
            )}
            <button type="button" className="btn" onClick={() => act('quality.ignore', { id: f.id })}>
              {t('quality.ignore')}
            </button>
          </li>
        ))}
      </ul>
    </div>
  )
}

function Duplicates() {
  const { t } = useTranslation()
  const [threshold, setThreshold] = useState(0.75)
  const { data, isLoading } = useQuery(q<DuplicateCandidate[]>('duplicates.find', { threshold, limit: 100 }))
  const { toast } = useApp()
  const merge = async (keep: string, remove: string) => {
    if (!window.confirm(t('duplicates.confirm'))) return
    const ok = await act('duplicates.merge', { keep, remove })
    if (ok) toast(t('duplicates.merged'), 'info', { label: t('action.undo'), run: () => void act('history.undo') })
  }
  return (
    <div>
      <div className="mb-3 flex items-center gap-3">
        <label className="label !mb-0" htmlFor="dup-th">
          {t('duplicates.threshold', { pct: Math.round(threshold * 100) })}
        </label>
        <input
          id="dup-th"
          type="range"
          min="0.5"
          max="0.95"
          step="0.05"
          value={threshold}
          onChange={(e) => setThreshold(Number(e.target.value))}
        />
      </div>
      {isLoading && <p className="text-[var(--muted)]">…</p>}
      {!isLoading && (data ?? []).length === 0 && (
        <p className="card p-6 text-center text-[var(--muted)]" data-testid="dup-none">
          {t('duplicates.none')}
        </p>
      )}
      <ul className="flex flex-col gap-3">
        {(data ?? []).map((c) => (
          <li key={c.a.id + c.b.id} className="card p-4" data-testid="dup-pair">
            <div className="mb-2 flex items-center gap-2 text-sm">
              <span className="chip">{Math.round(c.score * 100)}%</span>
              <span className="text-[var(--muted)]">{c.reasons.join(' · ')}</span>
            </div>
            <div className="grid gap-3 sm:grid-cols-2">
              {[c.a, c.b].map((p, i) => (
                <div key={p.id} className="rounded-lg border border-[var(--border)] p-3">
                  <div className="flex items-center gap-2">
                    <Avatar s={p} size={30} />
                    <div>
                      <div className="font-medium">{p.name}</div>
                      <div className="text-xs text-[var(--muted)]">{p.life}</div>
                    </div>
                  </div>
                  <dl className="mt-2 text-xs text-[var(--muted)]">
                    <div>
                      {t('event.kind.BIRT')}: {p.birth_text ?? '—'}
                    </div>
                    <div>
                      {t('event.kind.DEAT')}: {p.death_text ?? '—'}
                    </div>
                  </dl>
                  <button
                    type="button"
                    className="btn btn-primary mt-3"
                    onClick={() => merge(p.id, (i === 0 ? c.b : c.a).id)}
                  >
                    {t('duplicates.keepThis')}
                  </button>
                </div>
              ))}
            </div>
            <div className="mt-3">
              <button
                type="button"
                className="btn"
                onClick={() => act('duplicates.not_duplicate', { a: c.a.id, b: c.b.id })}
              >
                {t('duplicates.notDuplicate')}
              </button>
            </div>
          </li>
        ))}
      </ul>
    </div>
  )
}

export function Quality() {
  const { t } = useTranslation()
  return (
    <div className="mx-auto max-w-4xl p-6">
      <h1 className="mb-4 text-2xl font-semibold tracking-tight">{t('nav.quality')}</h1>
      <Tabs.Root defaultValue="plausibility">
        <Tabs.List className="mb-4 flex gap-1 border-b border-[var(--border)]">
          {['plausibility', 'duplicates'].map((k) => (
            <Tabs.Trigger
              key={k}
              value={k}
              className="px-3 py-2 text-sm data-[state=active]:border-b-2 data-[state=active]:border-[var(--accent)] data-[state=active]:font-medium data-[state=active]:text-[var(--accent)]"
            >
              {t(`quality.tab.${k}`)}
            </Tabs.Trigger>
          ))}
        </Tabs.List>
        <Tabs.Content value="plausibility">
          <Plausibility />
        </Tabs.Content>
        <Tabs.Content value="duplicates">
          <Duplicates />
        </Tabs.Content>
      </Tabs.Root>
    </div>
  )
}
