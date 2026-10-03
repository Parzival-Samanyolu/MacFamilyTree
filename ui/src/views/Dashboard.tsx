import { useQuery } from '@tanstack/react-query'
import { useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { bytesToBase64, call } from '../api/client'
import type { ImportReport, Status, Summary } from '../api/types'
import { PersonChip } from '../components/PersonChip'
import { act, q, refreshStatus } from '../lib/query'
import { useApp } from '../store/app'

interface DashData {
  on_this_day: { kind: string; year: number; years_ago: number; person: Summary }[]
  upcoming: { days: number; turning: number; person: Summary }[]
  random: Summary | null
  quality: { errors: number; warnings: number; info: number; score: number }
  bookmarks: Summary[]
  recent: Summary[]
}

export function Welcome() {
  const { t } = useTranslation()
  const { go, select, setWizard, toast } = useApp()
  const fileRef = useRef<HTMLInputElement>(null)
  const startNew = async () => {
    await act('project.new')
    setWizard(true)
  }
  const sample = async (persons: number) => {
    const r = await act<Status>('project.load_sample', { persons })
    if (r) {
      const list = await call<{ items: Summary[] }>('person.list', { limit: 1, sort: 'name' })
      if (list.items[0]) select(list.items[0].id)
      go('persons')
    }
  }
  const onFile = async (f: File | undefined) => {
    if (!f) return
    const bytes = new Uint8Array(await f.arrayBuffer())
    const r = await act<{ report: ImportReport }>('gedcom.import', { data: bytesToBase64(bytes) })
    if (r) {
      toast(t('import.done', { persons: r.report.persons, issues: r.report.issues.length }))
      go('importexport')
    }
  }
  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-6 p-8">
      <div>
        <h1 className="text-3xl font-semibold tracking-tight">{t('welcome.title')}</h1>
        <p className="mt-2 text-[var(--muted)]">{t('welcome.subtitle')}</p>
      </div>
      <div className="grid gap-4 sm:grid-cols-2">
        <button type="button" className="card p-5 text-left hover:bg-[var(--surface-2)]" onClick={startNew}>
          <div className="text-lg font-medium">{t('welcome.new')}</div>
          <div className="mt-1 text-sm text-[var(--muted)]">{t('welcome.newHint')}</div>
        </button>
        <button
          type="button"
          className="card p-5 text-left hover:bg-[var(--surface-2)]"
          onClick={() => fileRef.current?.click()}
        >
          <div className="text-lg font-medium">{t('welcome.import')}</div>
          <div className="mt-1 text-sm text-[var(--muted)]">{t('welcome.importHint')}</div>
        </button>
        <button type="button" className="card p-5 text-left hover:bg-[var(--surface-2)]" onClick={() => sample(30)}>
          <div className="text-lg font-medium">{t('welcome.sample')}</div>
          <div className="mt-1 text-sm text-[var(--muted)]">{t('welcome.sampleHint')}</div>
        </button>
        <button type="button" className="card p-5 text-left hover:bg-[var(--surface-2)]" onClick={() => sample(2000)}>
          <div className="text-lg font-medium">{t('welcome.sampleLarge')}</div>
          <div className="mt-1 text-sm text-[var(--muted)]">{t('welcome.sampleLargeHint')}</div>
        </button>
      </div>
      <input
        ref={fileRef}
        type="file"
        accept=".ged,.gedcom,text/plain"
        className="sr-only"
        aria-label={t('import.choose')}
        onChange={(e) => onFile(e.target.files?.[0])}
        data-testid="welcome-file"
      />
      <p className="text-xs text-[var(--muted)]">{t('welcome.privacy')}</p>
    </div>
  )
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="card p-4">
      <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-[var(--muted)]">{title}</h2>
      {children}
    </section>
  )
}

export function Dashboard() {
  const { t } = useTranslation()
  const { status, go, select } = useApp()
  const { data } = useQuery({ ...q<DashData>('dashboard.data'), enabled: status.open })
  if (!status.open) return <Welcome />
  const open = (id: string) => {
    select(id)
    go('persons', id)
  }
  const empty = <p className="text-sm text-[var(--muted)]">{t('dashboard.nothing')}</p>
  return (
    <div className="mx-auto grid max-w-5xl gap-4 p-6 md:grid-cols-2">
      <div className="md:col-span-2">
        <h1 className="text-2xl font-semibold tracking-tight">{t('dashboard.title')}</h1>
        <p className="text-sm text-[var(--muted)]">
          {t('dashboard.summary', { persons: status.persons ?? 0, families: status.families ?? 0 })}
        </p>
      </div>
      <Section title={t('dashboard.onThisDay')}>
        {data?.on_this_day.length ? (
          <ul className="flex flex-col gap-2">
            {data.on_this_day.map((x, i) => (
              <li key={i} className="flex items-center gap-2 text-sm">
                <span className="chip">{t(`event.kind.${x.kind}`)}</span>
                <PersonChip s={x.person} onClick={() => open(x.person.id)} />
                <span className="text-[var(--muted)]">{t('dashboard.yearsAgo', { count: x.years_ago })}</span>
              </li>
            ))}
          </ul>
        ) : (
          empty
        )}
      </Section>
      <Section title={t('dashboard.upcoming')}>
        {data?.upcoming.length ? (
          <ul className="flex flex-col gap-2">
            {data.upcoming.map((x, i) => (
              <li key={i} className="flex items-center gap-2 text-sm">
                <PersonChip s={x.person} onClick={() => open(x.person.id)} />
                <span className="text-[var(--muted)]">
                  {x.days === 0 ? t('dashboard.today') : t('dashboard.inDays', { count: x.days })}
                </span>
                <span className="chip">{t('dashboard.turning', { age: x.turning })}</span>
              </li>
            ))}
          </ul>
        ) : (
          empty
        )}
      </Section>
      <Section title={t('dashboard.quality')}>
        {data && (
          <div className="flex items-center gap-4">
            <div
              className="text-4xl font-semibold text-[var(--accent)]"
              aria-label={t('dashboard.qualityScore', { score: data.quality.score })}
            >
              {data.quality.score}
            </div>
            <div className="text-sm text-[var(--muted)]">
              <div>{t('dashboard.errors', { count: data.quality.errors })}</div>
              <div>{t('dashboard.warnings', { count: data.quality.warnings })}</div>
              <button type="button" className="btn mt-2" onClick={() => go('quality')}>
                {t('dashboard.review')}
              </button>
            </div>
          </div>
        )}
      </Section>
      <Section title={t('dashboard.randomAncestor')}>
        {data?.random ? <PersonChip s={data.random} onClick={() => open(data.random!.id)} /> : empty}
      </Section>
      <Section title={t('dashboard.bookmarks')}>
        {data?.bookmarks.length ? (
          <ul className="flex flex-col gap-2">
            {data.bookmarks.map((p) => (
              <li key={p.id}>
                <PersonChip s={p} onClick={() => open(p.id)} />
              </li>
            ))}
          </ul>
        ) : (
          empty
        )}
      </Section>
      <Section title={t('dashboard.recent')}>
        {data?.recent.length ? (
          <ul className="flex flex-col gap-2">
            {data.recent.map((p) => (
              <li key={p.id}>
                <PersonChip s={p} onClick={() => open(p.id)} />
              </li>
            ))}
          </ul>
        ) : (
          empty
        )}
      </Section>
    </div>
  )
}

export async function closeProject() {
  await call('project.close')
  await refreshStatus()
}
