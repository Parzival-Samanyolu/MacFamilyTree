import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { PersonDetail, Summary } from '../api/types'
import { act, q } from '../lib/query'
import { PersonChip } from './PersonChip'
import { PersonPicker } from './PersonPicker'

const ROLES = ['Godparent', 'Witness', 'Guardian', 'Friend', 'Neighbour', 'Employer', 'Teacher']

export function Associations({ d, open }: { d: PersonDetail; open: (id: string) => void }) {
  const { t } = useTranslation()
  const [other, setOther] = useState<Summary | null>(null)
  const [role, setRole] = useState('')
  const [notes, setNotes] = useState('')
  const id = d.summary.id
  const add = async () => {
    if (!other || !role.trim()) return
    await act('rec.put', {
      table: 'association',
      row: { person_id: id, other_id: other.id, role: role.trim(), notes: notes.trim() || null },
    })
    setOther(null)
    setRole('')
    setNotes('')
  }
  return (
    <section className="card p-4" data-testid="associations">
      <h2 className="mb-1 font-semibold">{t('assoc.title')}</h2>
      <p className="mb-3 text-sm text-[var(--muted)]">{t('assoc.hint')}</p>
      <ul className="mb-4 flex flex-col gap-2">
        {d.associations.length === 0 && <li className="text-sm text-[var(--muted)]">{t('assoc.none')}</li>}
        {d.associations.map((a) => (
          <li
            key={a.id}
            className="flex flex-wrap items-center gap-2 rounded-lg border border-[var(--border)] p-2 text-sm"
          >
            <span className="chip">{a.role}</span>
            <PersonChip s={a.other} onClick={() => open(a.other.id)} />
            {a.notes && <span className="text-[var(--muted)]">{a.notes}</span>}
            <span className="flex-1" />
            <button
              type="button"
              className="btn btn-danger"
              onClick={() => void act('rec.delete', { table: 'association', id: a.id })}
            >
              {t('common.delete')}
            </button>
          </li>
        ))}
      </ul>
      <div className="grid gap-3 md:grid-cols-3">
        <PersonPicker value={other} onChange={setOther} label={t('assoc.person')} exclude={[id]} />
        <div>
          <label className="label" htmlFor="assoc-role">
            {t('assoc.role')}
          </label>
          <input
            id="assoc-role"
            className="input"
            list="assoc-roles"
            value={role}
            onChange={(e) => setRole(e.target.value)}
          />
          <datalist id="assoc-roles">
            {ROLES.map((r) => (
              <option key={r} value={r} />
            ))}
          </datalist>
        </div>
        <div>
          <label className="label" htmlFor="assoc-notes">
            {t('assoc.notes')}
          </label>
          <input id="assoc-notes" className="input" value={notes} onChange={(e) => setNotes(e.target.value)} />
        </div>
      </div>
      <button
        type="button"
        className="btn btn-primary mt-3"
        disabled={!other || !role.trim()}
        onClick={() => void add()}
      >
        {t('common.add')}
      </button>
    </section>
  )
}

interface Change {
  id: number
  label: string
  ts: number
  undone: boolean
}

export function PersonHistory({ id }: { id: string }) {
  const { t, i18n } = useTranslation()
  const { data = [] } = useQuery(q<Change[]>('person.history', { id }))
  return (
    <section className="card p-4" data-testid="person-history">
      <h2 className="mb-1 font-semibold">{t('history.title')}</h2>
      <p className="mb-3 text-sm text-[var(--muted)]">{t('history.hint')}</p>
      {data.length === 0 && <p className="text-sm text-[var(--muted)]">{t('history.none')}</p>}
      <ol className="flex flex-col gap-1 text-sm">
        {data.map((c) => (
          <li key={c.id} className={`flex gap-3 ${c.undone ? 'text-[var(--muted)] line-through' : ''}`}>
            <time dateTime={new Date(c.ts * 1000).toISOString()} className="w-44 shrink-0 text-[var(--muted)]">
              {new Date(c.ts * 1000).toLocaleString(i18n.language)}
            </time>
            <span>{c.label}</span>
            {c.undone && <span className="chip no-underline">{t('history.undone')}</span>}
          </li>
        ))}
      </ol>
    </section>
  )
}
