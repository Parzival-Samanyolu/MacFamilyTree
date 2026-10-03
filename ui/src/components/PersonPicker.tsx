import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { Summary } from '../api/types'
import { q } from '../lib/query'
import { PersonChip } from './PersonChip'

/** Type-ahead search over all persons. */
export function PersonPicker({
  value,
  onChange,
  label,
  exclude,
}: {
  value: Summary | null
  onChange: (s: Summary | null) => void
  label: string
  exclude?: string[]
}) {
  const { t } = useTranslation()
  const [text, setText] = useState('')
  const { data } = useQuery({ ...q<Summary[]>('search', { q: text, limit: 8 }), enabled: text.trim().length > 0 })
  const results = (data ?? []).filter((r) => !exclude?.includes(r.id))
  if (value) {
    return (
      <div>
        <span className="label">{label}</span>
        <div className="flex items-center gap-2">
          <PersonChip s={value} />
          <button type="button" className="btn" onClick={() => onChange(null)} aria-label={t('common.clear')}>
            ✕
          </button>
        </div>
      </div>
    )
  }
  return (
    <div className="relative">
      <label className="label" htmlFor={`pp-${label}`}>
        {label}
      </label>
      <input
        id={`pp-${label}`}
        className="input"
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={t('picker.placeholder')}
        autoComplete="off"
      />
      {text.trim() && (
        <ul className="card absolute z-30 mt-1 max-h-64 w-full overflow-auto p-1" role="listbox" aria-label={label}>
          {results.length === 0 && <li className="px-2 py-1 text-sm text-[var(--muted)]">{t('common.noResults')}</li>}
          {results.map((r) => (
            <li key={r.id} role="option" aria-selected={false}>
              <button
                type="button"
                className="flex w-full items-center rounded-md px-2 py-1 text-left hover:bg-[var(--surface-2)]"
                onClick={() => {
                  onChange(r)
                  setText('')
                }}
              >
                <PersonChip s={r} />
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
