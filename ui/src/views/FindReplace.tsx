import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { call } from '../api/client'
import { act } from '../lib/query'
import { useApp } from '../store/app'

interface Change {
  table: string
  id: string
  before: string
  after: string
}
interface Preview {
  total: number
  changes: Change[]
}

const FIELDS = [
  'person_name.given',
  'person_name.surname',
  'person_name.nickname',
  'place.name',
  'event.description',
  'event.value',
  'event.cause',
  'note.body',
  'source.title',
  'source.author',
  'citation.page',
]

export function FindReplace() {
  const { t } = useTranslation()
  const { toast } = useApp()
  const [field, setField] = useState('person_name.surname')
  const [find, setFind] = useState('')
  const [replace, setReplace] = useState('')
  const [cs, setCs] = useState(false)
  const [whole, setWhole] = useState(false)
  const [pv, setPv] = useState<Preview | null>(null)
  const args = { field, find, replace, case_sensitive: cs, whole_field: whole }
  const doPreview = async () => {
    try {
      setPv(await call<Preview>('bulk.preview', { ...args, limit: 100 }))
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  const doApply = async () => {
    if (!pv || pv.total === 0 || !window.confirm(t('replace.confirm', { count: pv.total }))) return
    const r = await act<{ changed: number }>('bulk.apply', args)
    if (r) {
      toast(t('replace.done', { count: r.changed }), 'info', {
        label: t('action.undo'),
        run: () => void act('history.undo'),
      })
      setPv(null)
    }
  }
  return (
    <div className="mx-auto max-w-4xl p-6">
      <h1 className="mb-2 text-2xl font-semibold tracking-tight">{t('nav.replace')}</h1>
      <p className="mb-4 text-sm text-[var(--muted)]">{t('replace.hint')}</p>
      <div className="card grid gap-3 p-4 md:grid-cols-2">
        <div>
          <label className="label" htmlFor="fr-field">
            {t('replace.field')}
          </label>
          <select
            id="fr-field"
            className="input"
            value={field}
            onChange={(e) => (setField(e.target.value), setPv(null))}
          >
            {FIELDS.map((f) => (
              <option key={f} value={f}>
                {t(`replace.fields.${f.replace('.', '_')}`)}
              </option>
            ))}
          </select>
        </div>
        <div className="flex items-end gap-4 text-sm">
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={cs} onChange={(e) => (setCs(e.target.checked), setPv(null))} />
            {t('replace.caseSensitive')}
          </label>
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={whole} onChange={(e) => (setWhole(e.target.checked), setPv(null))} />
            {t('replace.whole')}
          </label>
        </div>
        <div>
          <label className="label" htmlFor="fr-find">
            {t('replace.find')}
          </label>
          <input id="fr-find" className="input" value={find} onChange={(e) => (setFind(e.target.value), setPv(null))} />
        </div>
        <div>
          <label className="label" htmlFor="fr-replace">
            {t('replace.with')}
          </label>
          <input
            id="fr-replace"
            className="input"
            value={replace}
            onChange={(e) => (setReplace(e.target.value), setPv(null))}
          />
        </div>
        <div className="flex gap-2 md:col-span-2">
          <button type="button" className="btn" disabled={!find} onClick={() => void doPreview()}>
            {t('replace.preview')}
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={!pv || pv.total === 0}
            onClick={() => void doApply()}
          >
            {t('replace.apply', { count: pv?.total ?? 0 })}
          </button>
        </div>
      </div>
      {pv && (
        <section className="mt-4" aria-live="polite" data-testid="replace-preview">
          <h2 className="mb-2 font-semibold">{t('replace.matches', { count: pv.total })}</h2>
          {pv.total > 0 && (
            <table className="w-full text-left text-sm">
              <thead>
                <tr>
                  <th scope="col">{t('replace.before')}</th>
                  <th scope="col">{t('replace.after')}</th>
                </tr>
              </thead>
              <tbody>
                {pv.changes.map((c) => (
                  <tr key={c.id}>
                    <td className="max-w-[24rem] truncate">{c.before}</td>
                    <td className="max-w-[24rem] truncate">{c.after}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {pv.total > pv.changes.length && (
            <p className="mt-1 text-xs text-[var(--muted)]">{t('replace.truncated', { shown: pv.changes.length })}</p>
          )}
        </section>
      )}
    </div>
  )
}
