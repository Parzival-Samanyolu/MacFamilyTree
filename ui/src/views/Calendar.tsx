import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { base64ToBytes, call } from '../api/client'
import type { Summary } from '../api/types'
import { downloadBlob } from '../lib/format'
import { q } from '../lib/query'
import { useApp } from '../store/app'

interface Day {
  day: number
  kind: 'BIRT' | 'DEAT' | 'MARR'
  person_id: string | null
  text: string
  years: number
  person?: Summary
}

export function CalendarView() {
  const { t, i18n } = useTranslation()
  const { select, go, toast } = useApp()
  const today = new Date()
  const [year, setYear] = useState(today.getFullYear())
  const [month, setMonth] = useState(today.getMonth() + 1)
  const [deceased, setDeceased] = useState(false)
  const { data } = useQuery(q<Day[]>('calendar.month', { year, month, include_deceased: deceased }))
  const first = new Date(year, month - 1, 1)
  const days = new Date(year, month, 0).getDate()
  const offset = (first.getDay() + 6) % 7 // Monday first
  const cells: (number | null)[] = [...Array(offset).fill(null), ...Array.from({ length: days }, (_, i) => i + 1)]
  while (cells.length % 7) cells.push(null)
  const by = new Map<number, Day[]>()
  for (const e of data ?? []) by.set(e.day, [...(by.get(e.day) ?? []), e])
  const monthName = new Intl.DateTimeFormat(i18n.language, { month: 'long', year: 'numeric' }).format(first)
  const weekdays = Array.from({ length: 7 }, (_, i) =>
    new Intl.DateTimeFormat(i18n.language, { weekday: 'short' }).format(new Date(2024, 0, 1 + i)),
  )
  const move = (d: number) => {
    const n = new Date(year, month - 1 + d, 1)
    setYear(n.getFullYear())
    setMonth(n.getMonth() + 1)
  }
  const exportIcs = async () => {
    const r = await call<{ data: string }>('export.ical', { include_deceased: deceased })
    downloadBlob('family-calendar.ics', base64ToBytes(r.data) as BlobPart, 'text/calendar')
    toast(t('calendar.exported'))
  }
  const tag = { BIRT: '🎂', DEAT: '✝', MARR: '💍' } as const
  return (
    <div className="mx-auto max-w-5xl p-6">
      <div className="mb-4 flex flex-wrap items-center gap-3">
        <h1 className="text-2xl font-semibold tracking-tight">{t('nav.calendar')}</h1>
        <div className="flex-1" />
        <button type="button" className="btn" onClick={() => move(-1)} aria-label={t('calendar.prev')}>
          ‹
        </button>
        <span className="min-w-40 text-center font-medium" data-testid="calendar-month">
          {monthName}
        </span>
        <button type="button" className="btn" onClick={() => move(1)} aria-label={t('calendar.next')}>
          ›
        </button>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={deceased} onChange={(e) => setDeceased(e.target.checked)} />{' '}
          {t('calendar.deceased')}
        </label>
        <button type="button" className="btn" onClick={exportIcs}>
          {t('calendar.export')}
        </button>
      </div>
      <table
        className="w-full table-fixed border-collapse overflow-hidden rounded-xl border border-[var(--border)]"
        aria-label={monthName}
      >
        <thead>
          <tr>
            {weekdays.map((w) => (
              <th
                key={w}
                scope="col"
                className="border border-[var(--border)] bg-[var(--surface-2)] px-2 py-1 text-center text-xs font-medium"
              >
                {w}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {Array.from({ length: cells.length / 7 }, (_, w) => (
            <tr key={w}>
              {cells.slice(w * 7, w * 7 + 7).map((d, i) => (
                <td
                  key={i}
                  className="h-24 border border-[var(--border)] bg-[var(--surface)] p-1.5 align-top"
                  data-day={d ?? undefined}
                >
                  {d && (
                    <>
                      <div
                        className={`mb-1 text-xs ${d === today.getDate() && month === today.getMonth() + 1 && year === today.getFullYear() ? 'font-bold text-[var(--accent)]' : 'text-[var(--muted)]'}`}
                      >
                        {d}
                      </div>
                      <ul className="flex flex-col gap-0.5">
                        {(by.get(d) ?? []).map((e, k) => (
                          <li key={k}>
                            <button
                              type="button"
                              className="w-full truncate rounded px-1 text-left text-xs hover:bg-[var(--accent-soft)]"
                              title={t(`calendar.${e.kind}`, { years: e.years })}
                              onClick={() => {
                                if (e.person_id) {
                                  select(e.person_id)
                                  go('persons', e.person_id)
                                }
                              }}
                            >
                              <span aria-hidden="true">{tag[e.kind]} </span>
                              {e.text}
                              {e.kind !== 'DEAT' && e.years > 0 ? ` (${e.years})` : ''}
                            </button>
                          </li>
                        ))}
                      </ul>
                    </>
                  )}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      <p className="mt-2 text-xs text-[var(--muted)]">{t('calendar.legend')}</p>
    </div>
  )
}
