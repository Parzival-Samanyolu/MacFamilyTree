import { useQuery } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { call } from '../api/client'
import type { EventItem } from '../api/types'
import { act, q } from '../lib/query'
import { Modal } from './Modal'

interface Meta {
  person: string[]
  family: string[]
}

interface Owner {
  type: 'person' | 'family'
  id: string
}

export function EventDialog({
  owner,
  event,
  onClose,
}: {
  owner: Owner
  event: EventItem | 'new' | null
  onClose: () => void
}) {
  const { t } = useTranslation()
  const editing = event && event !== 'new' ? event : null
  return (
    <Modal
      open={event !== null}
      onOpenChange={(o) => !o && onClose()}
      title={editing ? t('event.edit') : t('event.add')}
    >
      {/* keyed so the form is initialised from the event being edited and discarded on close */}
      {event !== null && <EventForm key={editing?.id ?? 'new'} owner={owner} editing={editing} onClose={onClose} />}
    </Modal>
  )
}

interface Parsed {
  text: string
  valid: boolean
  display?: string
  gedcom?: string
}

function EventForm({ owner, editing, onClose }: { owner: Owner; editing: EventItem | null; onClose: () => void }) {
  const { t } = useTranslation()
  const { data: meta } = useQuery(q<Meta>('meta.event_types'))
  const { data: places } = useQuery(q<{ id: string; full_name: string }[]>('place.list'))
  const [kind, setKind] = useState(editing?.kind ?? (owner.type === 'person' ? 'BIRT' : 'MARR'))
  const [custom, setCustom] = useState(editing?.custom_kind ?? '')
  const [date, setDate] = useState(editing?.date_gedcom ?? '')
  const [place, setPlace] = useState(editing?.place_text ?? '')
  const [value, setValue] = useState(editing?.value ?? '')
  const [cause, setCause] = useState(editing?.cause ?? '')
  const [parsed, setParsed] = useState<Parsed | null>(null)

  // live preview of how the core understood the typed date; state is only set from the async callback
  useEffect(() => {
    if (!date.trim()) return
    let live = true
    const h = setTimeout(() => {
      call<Omit<Parsed, 'text'>>('date.parse', { text: date })
        .then((r) => live && setParsed({ ...r, text: date }))
        .catch(() => undefined)
    }, 150)
    return () => {
      live = false
      clearTimeout(h)
    }
  }, [date])
  const preview = date.trim() && parsed?.text === date ? parsed : null

  const kinds = owner.type === 'person' ? meta?.person : meta?.family
  const submit = async () => {
    const args = {
      owner_type: owner.type,
      owner_id: owner.id,
      kind,
      custom_kind: custom,
      date_text: date,
      place_text: place,
      value,
      cause,
    }
    const r = await act('event.put', editing ? { ...args, id: editing.id } : args)
    if (r) onClose()
  }
  const remove = async () => {
    if (editing && (await act('event.delete', { id: editing.id }))) onClose()
  }

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault()
        void submit()
      }}
      className="grid grid-cols-2 gap-3"
    >
      <div>
        <label className="label" htmlFor="ev-kind">
          {t('event.type')}
        </label>
        <select id="ev-kind" className="input" value={kind} onChange={(e) => setKind(e.target.value)}>
          {(kinds ?? [kind]).map((k) => (
            <option key={k} value={k}>
              {t(`event.kind.${k}`)}
            </option>
          ))}
        </select>
      </div>
      <div>
        <label className="label" htmlFor="ev-custom">
          {t('event.customType')}
        </label>
        <input id="ev-custom" className="input" value={custom} onChange={(e) => setCustom(e.target.value)} />
      </div>
      <div className="col-span-2">
        <label className="label" htmlFor="ev-date">
          {t('event.date')}
        </label>
        <input
          id="ev-date"
          className="input"
          value={date}
          placeholder={t('event.datePlaceholder')}
          onChange={(e) => setDate(e.target.value)}
          autoFocus
        />
        <div className="mt-1 min-h-5 text-xs" aria-live="polite" data-testid="date-preview">
          {preview &&
            (preview.valid ? (
              <span className="text-[var(--accent)]">
                {preview.display || ''}
                {preview.gedcom ? `  (${preview.gedcom})` : ''}
              </span>
            ) : (
              <span className="text-[var(--warn)]">{t('event.dateInvalid')}</span>
            ))}
        </div>
      </div>
      <div className="col-span-2">
        <label className="label" htmlFor="ev-place">
          {t('event.place')}
        </label>
        <input
          id="ev-place"
          className="input"
          value={place}
          list="place-options"
          placeholder={t('event.placePlaceholder')}
          onChange={(e) => setPlace(e.target.value)}
        />
        <datalist id="place-options">
          {(places ?? []).slice(0, 400).map((p) => (
            <option key={p.id} value={p.full_name} />
          ))}
        </datalist>
      </div>
      <div>
        <label className="label" htmlFor="ev-value">
          {t('event.value')}
        </label>
        <input id="ev-value" className="input" value={value} onChange={(e) => setValue(e.target.value)} />
      </div>
      <div>
        <label className="label" htmlFor="ev-cause">
          {t('event.cause')}
        </label>
        <input id="ev-cause" className="input" value={cause} onChange={(e) => setCause(e.target.value)} />
      </div>
      <div className="col-span-2 mt-2 flex items-center gap-2">
        {editing && (
          <button type="button" className="btn btn-danger" onClick={remove}>
            {t('common.delete')}
          </button>
        )}
        <div className="flex-1" />
        <button type="button" className="btn" onClick={onClose}>
          {t('common.cancel')}
        </button>
        <button type="submit" className="btn btn-primary">
          {t('common.save')}
        </button>
      </div>
    </form>
  )
}
