import * as Tabs from '@radix-ui/react-tabs'
import * as Menu from '@radix-ui/react-dropdown-menu'
import { useQuery } from '@tanstack/react-query'
import { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { call } from '../api/client'
import type { EventItem, NameItem, PersonDetail, Summary } from '../api/types'
import { act, q } from '../lib/query'
import { useApp } from '../store/app'
import { EventDialog } from './EventDialog'
import { Icons } from './Icons'
import { Modal } from './Modal'
import { Avatar, PersonChip } from './PersonChip'
import { PersonPicker } from './PersonPicker'
import { MediaThumb } from './MediaThumb'
import { Associations, PersonHistory } from './PersonExtras'
import { uploadFiles, type MediaItem } from '../lib/media'

const FAM_REL = ['married', 'partners', 'divorced', 'separated', 'unmarried', 'friends', 'unknown']
const REL_KINDS = ['father', 'mother', 'partner', 'child', 'sibling'] as const
type RelKind = (typeof REL_KINDS)[number]

export function PersonEditor({ id }: { id: string }) {
  const { t } = useTranslation()
  const { data, error } = useQuery(q<PersonDetail>('person.get', { id }))
  const { go, select } = useApp()
  const [addKind, setAddKind] = useState<{ kind: RelKind; family_id?: string } | null>(null)
  const [tab, setTab] = useState('overview')
  if (error) return <div className="p-6 text-[var(--danger)]">{String((error as Error).message)}</div>
  if (!data) return <div className="p-6 text-[var(--muted)]">…</div>
  const p = data.person
  const s = data.summary
  const open = (pid: string) => select(pid)
  const del = async () => {
    if (!window.confirm(t('person.confirmDelete', { name: s.name || t('person.unnamed') }))) return
    await act('person.delete', { id })
    useApp.getState().select(null)
    useApp
      .getState()
      .toast(t('toast.deleted'), 'info', { label: t('action.undo'), run: () => void act('history.undo') })
  }
  return (
    <div className="mx-auto max-w-4xl p-5" data-testid="person-editor">
      <header className="mb-4 flex items-center gap-4">
        <Avatar s={s} size={56} />
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-2xl font-semibold tracking-tight" data-testid="person-title">
            {s.name || t('person.unnamed')}
          </h1>
          <div className="flex flex-wrap items-center gap-2 text-sm text-[var(--muted)]">
            <span>{s.life}</span>
            <span className="chip">{t(`sex.${p.sex || 'U'}`)}</span>
            {p.living && <span className="chip">{t('person.living')}</span>}
            {p.is_private && <span className="chip">{t('person.private')}</span>}
          </div>
        </div>
        <button
          type="button"
          className={`btn ${p.bookmarked ? 'text-[var(--warn)]' : ''}`}
          aria-pressed={p.bookmarked}
          aria-label={t('person.bookmark')}
          onClick={() => act('person.update', { id, bookmarked: !p.bookmarked })}
        >
          {Icons.star}
        </button>
        <button type="button" className="btn" onClick={() => go('tree', id)}>
          {Icons.tree} {t('person.showInTree')}
        </button>
        <Menu.Root>
          <Menu.Trigger className="btn btn-primary">
            {Icons.plus} {t('person.addRelative')}
          </Menu.Trigger>
          <Menu.Portal>
            <Menu.Content className="card z-50 min-w-40 p-1" sideOffset={6}>
              {REL_KINDS.map((k) => (
                <Menu.Item
                  key={k}
                  className="cursor-pointer rounded-md px-3 py-1.5 text-sm outline-none data-[highlighted]:bg-[var(--surface-2)]"
                  onSelect={() => setAddKind({ kind: k })}
                >
                  {t(`relative.${k}`)}
                </Menu.Item>
              ))}
            </Menu.Content>
          </Menu.Portal>
        </Menu.Root>
        <button type="button" className="btn btn-danger" onClick={del}>
          {t('common.delete')}
        </button>
      </header>

      <Tabs.Root value={tab} onValueChange={setTab}>
        <Tabs.List className="mb-4 flex gap-1 border-b border-[var(--border)]" aria-label={t('person.sections')}>
          {['overview', 'names', 'events', 'relationships', 'associations', 'media', 'notes', 'history'].map((k) => (
            <Tabs.Trigger
              key={k}
              value={k}
              className="rounded-t-md px-3 py-2 text-sm data-[state=active]:border-b-2 data-[state=active]:border-[var(--accent)] data-[state=active]:font-medium data-[state=active]:text-[var(--accent)]"
            >
              {t(`tab.${k}`)}
              {k === 'events' && <span className="ml-1 text-xs text-[var(--muted)]">{data.events.length}</span>}
            </Tabs.Trigger>
          ))}
        </Tabs.List>
        <Tabs.Content value="overview">
          <Overview d={data} open={open} />
        </Tabs.Content>
        <Tabs.Content value="names">
          <Names d={data} />
        </Tabs.Content>
        <Tabs.Content value="events">
          <Events d={data} />
        </Tabs.Content>
        <Tabs.Content value="relationships">
          <Relationships d={data} open={open} onAdd={(kind, family_id) => setAddKind({ kind, family_id })} />
        </Tabs.Content>
        <Tabs.Content value="associations">
          <Associations d={data} open={open} />
        </Tabs.Content>
        <Tabs.Content value="history">
          <PersonHistory id={id} />
        </Tabs.Content>
        <Tabs.Content value="media">
          <PersonMedia id={id} primary={data.person.primary_media ?? null} />
        </Tabs.Content>
        <Tabs.Content value="notes">
          <NotesSources d={data} />
        </Tabs.Content>
      </Tabs.Root>
      <AddRelativeDialog person={s} spec={addKind} onClose={() => setAddKind(null)} />
    </div>
  )
}

function Field({ label, children, id }: { label: string; children: React.ReactNode; id: string }) {
  return (
    <div>
      <label className="label" htmlFor={id}>
        {label}
      </label>
      {children}
    </div>
  )
}

function Overview({ d, open }: { d: PersonDetail; open: (id: string) => void }) {
  const { t } = useTranslation()
  const p = d.person
  const key = (e: EventItem) => ['BIRT', 'DEAT', 'BURI', 'OCCU'].includes(e.kind)
  return (
    <div className="grid gap-4 md:grid-cols-2">
      <section className="card grid gap-3 p-4">
        <Field label={t('person.sex')} id="ov-sex">
          <select
            id="ov-sex"
            className="input"
            value={p.sex || 'U'}
            onChange={(e) => act('person.update', { id: p.id, sex: e.target.value })}
          >
            {['M', 'F', 'X', 'U'].map((x) => (
              <option key={x} value={x}>
                {t(`sex.${x}`)}
              </option>
            ))}
          </select>
        </Field>
        <Field label={t('person.refNo')} id="ov-ref">
          <input
            id="ov-ref"
            className="input"
            defaultValue={p.ref_no ?? ''}
            onBlur={(e) =>
              e.target.value !== (p.ref_no ?? '') && act('person.update', { id: p.id, ref_no: e.target.value })
            }
          />
        </Field>
        <Field label={t('person.livingStatus')} id="ov-living">
          <select
            id="ov-living"
            className="input"
            value={p.living_override === null ? 'auto' : p.living_override ? 'yes' : 'no'}
            onChange={(e) =>
              act('person.update', {
                id: p.id,
                living_override: e.target.value === 'auto' ? null : e.target.value === 'yes',
              })
            }
          >
            <option value="auto">{t('person.livingAuto')}</option>
            <option value="yes">{t('person.livingYes')}</option>
            <option value="no">{t('person.livingNo')}</option>
          </select>
        </Field>
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={p.is_private}
            onChange={(e) => act('person.update', { id: p.id, is_private: e.target.checked })}
          />
          {t('person.privateFlag')}
        </label>
        <Field label={t('person.color')} id="ov-color">
          <input
            id="ov-color"
            type="color"
            className="h-9 w-16 rounded border border-[var(--border)]"
            value={p.color ?? '#2f6f5e'}
            onChange={(e) => act('person.update', { id: p.id, color: e.target.value })}
          />
        </Field>
      </section>
      <section className="card p-4">
        <h2 className="mb-2 text-sm font-semibold uppercase tracking-wide text-[var(--muted)]">
          {t('person.keyFacts')}
        </h2>
        <ul className="flex flex-col gap-1.5 text-sm">
          {d.events.filter(key).map((e) => (
            <li key={e.id}>
              <span className="chip mr-2">{t(`event.kind.${e.kind}`)}</span>
              {[e.date_text, e.place_text, e.value].filter(Boolean).join(' · ') || '—'}
            </li>
          ))}
          {!d.events.some(key) && <li className="text-[var(--muted)]">{t('dashboard.nothing')}</li>}
        </ul>
        <h2 className="mb-2 mt-4 text-sm font-semibold uppercase tracking-wide text-[var(--muted)]">
          {t('tab.relationships')}
        </h2>
        <div className="flex flex-wrap gap-2">
          {d.child_families
            .flatMap((f) => f.parents)
            .map((x) => (
              <PersonChip key={x.id} s={x} onClick={() => open(x.id)} />
            ))}
          {d.partner_families
            .flatMap((f) => f.partners)
            .map((x) => (
              <PersonChip key={x.id} s={x} onClick={() => open(x.id)} />
            ))}
          {d.partner_families
            .flatMap((f) => f.children)
            .map((x) => (
              <PersonChip key={x.id} s={x} onClick={() => open(x.id)} />
            ))}
        </div>
      </section>
    </div>
  )
}

const NAME_KINDS = ['Birth', 'Married', 'Aka', 'Religious', 'Immigrant']

function Names({ d }: { d: PersonDetail }) {
  const { t } = useTranslation()
  const save = (n: NameItem, patch: Partial<NameItem>) =>
    void act('name.put', { ...patch, id: n.id, person_id: d.person.id })
  const fields: [keyof NameItem, string][] = [
    ['prefix', 'name.prefix'],
    ['given', 'person.given'],
    ['nickname', 'name.nickname'],
    ['surname_prefix', 'name.surnamePrefix'],
    ['surname', 'person.surname'],
    ['suffix', 'name.suffix'],
  ]
  return (
    <div className="flex flex-col gap-3">
      {d.names.map((n, i) => (
        <section key={n.id} className="card p-4" data-testid="name-card">
          <div className="mb-2 flex items-center gap-2">
            <select
              className="input !w-auto"
              aria-label={t('name.type')}
              value={NAME_KINDS.includes(n.kind) ? n.kind : n.kind || 'Birth'}
              onChange={(e) => save(n, { kind: e.target.value })}
            >
              {NAME_KINDS.map((k) => (
                <option key={k} value={k}>
                  {t(`name.kind.${k}`)}
                </option>
              ))}
              {!NAME_KINDS.includes(n.kind) && n.kind && <option value={n.kind}>{n.kind}</option>}
            </select>
            {i === 0 ? (
              <span className="chip">{t('name.primary')}</span>
            ) : (
              <button
                type="button"
                className="btn"
                onClick={() => act('name.set_primary', { person_id: d.person.id, id: n.id })}
              >
                {t('name.makePrimary')}
              </button>
            )}
            <div className="flex-1" />
            {d.names.length > 1 && (
              <button type="button" className="btn btn-danger" onClick={() => act('name.delete', { id: n.id })}>
                {t('common.delete')}
              </button>
            )}
          </div>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-3">
            {fields.map(([k, label]) => (
              <Field key={k} label={t(label)} id={`n-${n.id}-${k}`}>
                <input
                  id={`n-${n.id}-${k}`}
                  className="input"
                  defaultValue={String(n[k] ?? '')}
                  onBlur={(e) => e.target.value !== String(n[k] ?? '') && save(n, { [k]: e.target.value })}
                />
              </Field>
            ))}
          </div>
        </section>
      ))}
      <div>
        <button
          type="button"
          className="btn"
          onClick={() => act('name.put', { person_id: d.person.id, kind: 'Aka', given: '', surname: '' })}
        >
          {Icons.plus} {t('name.add')}
        </button>
      </div>
    </div>
  )
}

function Events({ d }: { d: PersonDetail }) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState<EventItem | 'new' | null>(null)
  const [drag, setDrag] = useState<string | null>(null)
  const reorder = (over: string) => {
    if (!drag || drag === over) return
    const ids = d.events.map((e) => e.id)
    ids.splice(ids.indexOf(drag), 1)
    ids.splice(ids.indexOf(over), 0, drag)
    void act('event.reorder', { ids })
    setDrag(null)
  }
  return (
    <div>
      <div className="mb-3 flex items-center justify-between">
        <p className="text-sm text-[var(--muted)]">{t('event.hint')}</p>
        <button type="button" className="btn btn-primary" onClick={() => setEditing('new')}>
          {Icons.plus} {t('event.add')}
        </button>
      </div>
      <div className="card overflow-hidden">
        <table className="w-full text-sm" data-testid="events-table">
          <thead className="bg-[var(--surface-2)] text-left">
            <tr>
              <th className="w-6">
                <span className="sr-only">{t('event.reorder')}</span>
              </th>
              <th className="px-3 py-2">{t('event.type')}</th>
              <th className="px-3 py-2">{t('event.date')}</th>
              <th className="px-3 py-2">{t('event.age')}</th>
              <th className="px-3 py-2">{t('event.place')}</th>
              <th className="px-3 py-2">{t('event.details')}</th>
            </tr>
          </thead>
          <tbody>
            {d.events.length === 0 && (
              <tr>
                <td colSpan={6} className="px-3 py-4 text-[var(--muted)]">
                  {t('event.none')}
                </td>
              </tr>
            )}
            {d.events.map((e) => (
              <tr
                key={e.id}
                draggable
                onDragStart={() => setDrag(e.id)}
                onDragOver={(ev) => ev.preventDefault()}
                onDrop={() => reorder(e.id)}
                className="cursor-pointer border-t border-[var(--border)] hover:bg-[var(--surface-2)]"
                onDoubleClick={() => setEditing(e)}
              >
                <td className="px-1 text-[var(--muted)]" aria-hidden="true">
                  ⋮⋮
                </td>
                <td className="px-3 py-2">
                  <button
                    type="button"
                    className="font-medium text-[var(--accent)] hover:underline"
                    onClick={() => setEditing(e)}
                  >
                    {e.kind === 'EVEN' && e.custom_kind ? e.custom_kind : t(`event.kind.${e.kind}`)}
                  </button>
                </td>
                <td className="px-3 py-2">{e.date_text ?? ''}</td>
                <td className="px-3 py-2">{e.age !== null ? e.age : ''}</td>
                <td className="px-3 py-2">{e.place_text ?? ''}</td>
                <td className="px-3 py-2">
                  {[e.value, e.cause].filter(Boolean).join(' · ')}
                  {e.citations.length > 0 && (
                    <span className="chip ml-2" title={t('tab.notes')}>
                      §{e.citations.length}
                    </span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <EventDialog owner={{ type: 'person', id: d.person.id }} event={editing} onClose={() => setEditing(null)} />
    </div>
  )
}

function Relationships({
  d,
  open,
  onAdd,
}: {
  d: PersonDetail
  open: (id: string) => void
  onAdd: (k: RelKind, family?: string) => void
}) {
  const { t } = useTranslation()
  const [link, setLink] = useState<{ role: 'parent' | 'partner' | 'child'; family?: string } | null>(null)
  const [dragChild, setDragChild] = useState<{ fam: string; id: string } | null>(null)
  const REL = ['', 'adopted', 'foster', 'step', 'sealed']
  const dropChild = (fam: string, ids: string[], over: string) => {
    if (!dragChild || dragChild.fam !== fam || dragChild.id === over) return
    const list = [...ids]
    list.splice(list.indexOf(dragChild.id), 1)
    list.splice(list.indexOf(over), 0, dragChild.id)
    void act('family.reorder_children', { ids: list })
    setDragChild(null)
  }
  return (
    <div className="flex flex-col gap-4">
      <section className="card p-4">
        <div className="mb-2 flex items-center justify-between">
          <h2 className="font-semibold">{t('rel.parents')}</h2>
          <div className="flex gap-2">
            <button type="button" className="btn" onClick={() => onAdd('father')}>
              {t('relative.father')}
            </button>
            <button type="button" className="btn" onClick={() => onAdd('mother')}>
              {t('relative.mother')}
            </button>
            <button type="button" className="btn" onClick={() => setLink({ role: 'parent' })}>
              {t('rel.linkExisting')}
            </button>
          </div>
        </div>
        {d.child_families.length === 0 && <p className="text-sm text-[var(--muted)]">{t('rel.noParents')}</p>}
        {d.child_families.map((f) => (
          <div key={f.id} className="mb-3 rounded-lg border border-[var(--border)] p-3">
            <div className="flex flex-wrap items-center gap-2">
              {f.parents.map((x) => (
                <PersonChip key={x.id} s={x} onClick={() => open(x.id)} />
              ))}
              <div className="flex-1" />
              <label className="text-xs text-[var(--muted)]" htmlFor={`cl-${f.link_id}`}>
                {t('rel.linkType')}
              </label>
              <select
                id={`cl-${f.link_id}`}
                className="input !w-auto"
                value={f.rel_type}
                onChange={(e) => act('family.set_child', { id: f.link_id, rel_type: e.target.value })}
              >
                {REL.map((r) => (
                  <option key={r} value={r}>
                    {t(`childrel.${r || 'biological'}`)}
                  </option>
                ))}
              </select>
              <button type="button" className="btn" onClick={() => act('family.remove_child', { id: f.link_id })}>
                {t('rel.unlink')}
              </button>
            </div>
            {f.siblings.length > 0 && (
              <div className="mt-2 flex flex-wrap items-center gap-2 text-sm">
                <span className="text-[var(--muted)]">{t('rel.siblings')}:</span>
                {f.siblings.map((x) => (
                  <PersonChip key={x.id} s={x} onClick={() => open(x.id)} />
                ))}
              </div>
            )}
          </div>
        ))}
        <button type="button" className="btn" onClick={() => onAdd('sibling')}>
          {Icons.plus} {t('relative.sibling')}
        </button>
      </section>

      <section className="card p-4">
        <div className="mb-2 flex items-center justify-between">
          <h2 className="font-semibold">{t('rel.partners')}</h2>
          <div className="flex gap-2">
            <button type="button" className="btn" onClick={() => onAdd('partner')}>
              {Icons.plus} {t('relative.partner')}
            </button>
            <button type="button" className="btn" onClick={() => setLink({ role: 'partner' })}>
              {t('rel.linkExisting')}
            </button>
          </div>
        </div>
        {d.partner_families.length === 0 && <p className="text-sm text-[var(--muted)]">{t('rel.noPartners')}</p>}
        {d.partner_families.map((f) => {
          const ids = f.children.map((c) => c.link_id)
          return (
            <div key={f.id} className="mb-3 rounded-lg border border-[var(--border)] p-3" data-testid="partner-family">
              <div className="flex flex-wrap items-center gap-2">
                {f.partners.length === 0 && (
                  <span className="text-sm text-[var(--muted)]">{t('rel.unknownPartner')}</span>
                )}
                {f.partners.map((x) => (
                  <PersonChip key={x.id} s={x} onClick={() => open(x.id)} />
                ))}
                <div className="flex-1" />
                <select
                  className="input !w-auto"
                  aria-label={t('rel.type')}
                  value={f.rel_type}
                  onChange={(e) => act('family.update', { id: f.id, rel_type: e.target.value })}
                >
                  {FAM_REL.map((r) => (
                    <option key={r} value={r}>
                      {t(`famrel.${r}`)}
                    </option>
                  ))}
                  {!FAM_REL.includes(f.rel_type) && (
                    <option value={f.rel_type}>{f.rel_type || t('famrel.unknown')}</option>
                  )}
                </select>
                <button
                  type="button"
                  className="btn btn-danger"
                  onClick={() => window.confirm(t('rel.confirmFamily')) && act('family.delete', { id: f.id })}
                >
                  {t('rel.deleteFamily')}
                </button>
              </div>
              {f.events.length > 0 && (
                <div className="mt-2 text-sm text-[var(--muted)]">
                  {f.events
                    .map((e) => `${t(`event.kind.${e.kind}`)} ${e.date_text ?? ''} ${e.place_text ?? ''}`.trim())
                    .join(' · ')}
                </div>
              )}
              <ul className="mt-3 flex flex-col gap-1.5">
                {f.children.map((c) => (
                  <li
                    key={c.link_id}
                    draggable
                    onDragStart={() => setDragChild({ fam: f.id, id: c.link_id })}
                    onDragOver={(e) => e.preventDefault()}
                    onDrop={() => dropChild(f.id, ids, c.link_id)}
                    className="flex items-center gap-2"
                  >
                    <span className="text-[var(--muted)]" aria-hidden="true">
                      ⋮⋮
                    </span>
                    <PersonChip s={c} onClick={() => open(c.id)} />
                    {c.rel_type && <span className="chip">{t(`childrel.${c.rel_type}`)}</span>}
                  </li>
                ))}
              </ul>
              <div className="mt-3 flex gap-2">
                <button type="button" className="btn" onClick={() => onAdd('child', f.id)}>
                  {Icons.plus} {t('relative.child')}
                </button>
                <button type="button" className="btn" onClick={() => setLink({ role: 'child', family: f.id })}>
                  {t('rel.linkExisting')}
                </button>
              </div>
            </div>
          )
        })}
      </section>
      <LinkDialog person={d.summary} spec={link} onClose={() => setLink(null)} />
    </div>
  )
}

function LinkDialog({
  person,
  spec,
  onClose,
}: {
  person: Summary
  spec: { role: 'parent' | 'partner' | 'child'; family?: string } | null
  onClose: () => void
}) {
  const { t } = useTranslation()
  const [other, setOther] = useState<Summary | null>(null)
  const submit = async () => {
    if (!spec || !other) return
    if (spec.role === 'partner') await act('family.create', { partner1: person.id, partner2: other.id })
    else if (spec.role === 'child') {
      if (spec.family) await act('family.add_child', { family_id: spec.family, person_id: other.id })
    } else {
      // existing person becomes a parent of `person`: reuse the person's parent family when there is one
      const detail = await call<PersonDetail>('person.get', { id: person.id })
      const fam = detail.child_families[0]
      if (fam)
        await act('family.update', {
          id: fam.id,
          [detail.child_families[0].parents.length ? 'partner2' : 'partner1']: other.id,
        })
      else {
        const f = await act<{ id: string }>('family.create', { partner1: other.id, rel_type: 'unknown' })
        if (f) await act('family.add_child', { family_id: f.id, person_id: person.id })
      }
    }
    setOther(null)
    onClose()
  }
  return (
    <Modal open={!!spec} onOpenChange={(o) => !o && onClose()} title={t('rel.linkExisting')}>
      <PersonPicker label={t('rel.choosePerson')} value={other} onChange={setOther} exclude={[person.id]} />
      <div className="mt-4 flex justify-end gap-2">
        <button type="button" className="btn" onClick={onClose}>
          {t('common.cancel')}
        </button>
        <button type="button" className="btn btn-primary" disabled={!other} onClick={submit}>
          {t('rel.link')}
        </button>
      </div>
    </Modal>
  )
}

function AddRelativeDialog({
  person,
  spec,
  onClose,
}: {
  person: Summary
  spec: { kind: RelKind; family_id?: string } | null
  onClose: () => void
}) {
  const { t } = useTranslation()
  const [given, setGiven] = useState('')
  const [surname, setSurname] = useState('')
  const [sex, setSex] = useState('')
  const { select } = useApp()
  if (!spec)
    return (
      <Modal open={false} onOpenChange={onClose} title="">
        {null}
      </Modal>
    )
  const defaultSex = spec.kind === 'father' ? 'M' : spec.kind === 'mother' ? 'F' : 'U'
  const submit = async () => {
    const r = await act<{ person_id: string }>('relative.add', {
      person_id: person.id,
      kind: spec.kind,
      family_id: spec.family_id,
      given,
      surname: surname || undefined,
      sex: sex || defaultSex,
    })
    if (r) {
      setGiven('')
      setSurname('')
      setSex('')
      onClose()
      useApp.getState().toast(t('toast.added', { name: given || t('person.unnamed') }), 'info', {
        label: t('action.open'),
        run: () => select(r.person_id),
      })
    }
  }
  return (
    <Modal
      open={!!spec}
      onOpenChange={(o) => !o && onClose()}
      title={`${t('person.addRelative')}: ${t(`relative.${spec.kind}`)}`}
    >
      <p className="mb-3 text-sm text-[var(--muted)]">
        {t('relative.hint', { name: person.name || t('person.unnamed') })}
      </p>
      <form
        onSubmit={(e) => {
          e.preventDefault()
          void submit()
        }}
        className="grid grid-cols-2 gap-3"
      >
        <Field label={t('person.given')} id="ar-given">
          <input id="ar-given" className="input" value={given} onChange={(e) => setGiven(e.target.value)} autoFocus />
        </Field>
        <Field label={t('person.surname')} id="ar-surname">
          <input
            id="ar-surname"
            className="input"
            value={surname}
            placeholder={t('relative.surnameAuto')}
            onChange={(e) => setSurname(e.target.value)}
          />
        </Field>
        <Field label={t('person.sex')} id="ar-sex">
          <select id="ar-sex" className="input" value={sex || defaultSex} onChange={(e) => setSex(e.target.value)}>
            {['M', 'F', 'X', 'U'].map((x) => (
              <option key={x} value={x}>
                {t(`sex.${x}`)}
              </option>
            ))}
          </select>
        </Field>
        <div className="col-span-2 flex justify-end gap-2">
          <button type="button" className="btn" onClick={onClose}>
            {t('common.cancel')}
          </button>
          <button type="submit" className="btn btn-primary">
            {t('common.add')}
          </button>
        </div>
      </form>
    </Modal>
  )
}

function NotesSources({ d }: { d: PersonDetail }) {
  const { t } = useTranslation()
  const [note, setNote] = useState('')
  const [title, setTitle] = useState('')
  const [page, setPage] = useState('')
  const [existing, setExisting] = useState('')
  const { data: sources } = useQuery(q<{ id: string; title: string }[]>('rec.list', { table: 'source' }))
  const addNote = async () => {
    if (!note.trim()) return
    await act('note.add', { target_type: 'person', target_id: d.person.id, body: note })
    setNote('')
  }
  const addCite = async () => {
    if (!existing && !title.trim()) return
    await act('citation.add', {
      target_type: 'person',
      target_id: d.person.id,
      source_id: existing || undefined,
      new_source_title: existing ? undefined : title,
      page,
    })
    setTitle('')
    setPage('')
    setExisting('')
  }
  return (
    <div className="grid gap-4 md:grid-cols-2">
      <section className="card p-4">
        <h2 className="mb-2 font-semibold">{t('notes.title')}</h2>
        <ul className="mb-3 flex flex-col gap-2">
          {d.notes.length === 0 && <li className="text-sm text-[var(--muted)]">{t('notes.none')}</li>}
          {d.notes.map((n) => (
            <li key={n.id} className="rounded-lg border border-[var(--border)] p-2 text-sm">
              <textarea
                className="input min-h-16"
                defaultValue={n.body}
                aria-label={t('notes.title')}
                onBlur={(e) => e.target.value !== n.body && act('note.update', { id: n.id, body: e.target.value })}
              />
              <button
                type="button"
                className="btn btn-danger mt-1"
                onClick={() => act('note.remove', { link_id: n.link_id })}
              >
                {t('common.delete')}
              </button>
            </li>
          ))}
        </ul>
        <label className="label" htmlFor="new-note">
          {t('notes.add')}
        </label>
        <textarea id="new-note" className="input min-h-16" value={note} onChange={(e) => setNote(e.target.value)} />
        <button type="button" className="btn btn-primary mt-2" onClick={addNote} disabled={!note.trim()}>
          {t('common.add')}
        </button>
      </section>
      <section className="card p-4">
        <h2 className="mb-2 font-semibold">{t('sources.title')}</h2>
        <ul className="mb-3 flex flex-col gap-2">
          {d.citations.length === 0 && <li className="text-sm text-[var(--muted)]">{t('sources.none')}</li>}
          {d.citations.map((c) => (
            <li key={c.id} className="flex items-center gap-2 rounded-lg border border-[var(--border)] p-2 text-sm">
              <span className="min-w-0 flex-1 truncate">
                § {c.source_title}
                {c.page ? ` — ${c.page}` : ''}
              </span>
              <button
                type="button"
                className="btn btn-danger"
                onClick={() => act('rec.delete', { table: 'citation', id: c.id })}
              >
                {t('common.delete')}
              </button>
            </li>
          ))}
        </ul>
        <div className="grid gap-2">
          <div>
            <label className="label" htmlFor="cite-src">
              {t('sources.existing')}
            </label>
            <select id="cite-src" className="input" value={existing} onChange={(e) => setExisting(e.target.value)}>
              <option value="">{t('sources.newSource')}</option>
              {(sources ?? []).map((s) => (
                <option key={s.id} value={s.id}>
                  {s.title}
                </option>
              ))}
            </select>
          </div>
          {!existing && (
            <div>
              <label className="label" htmlFor="cite-title">
                {t('sources.titleLabel')}
              </label>
              <input id="cite-title" className="input" value={title} onChange={(e) => setTitle(e.target.value)} />
            </div>
          )}
          <div>
            <label className="label" htmlFor="cite-page">
              {t('sources.page')}
            </label>
            <input id="cite-page" className="input" value={page} onChange={(e) => setPage(e.target.value)} />
          </div>
          <button
            type="button"
            className="btn btn-primary justify-self-start"
            onClick={addCite}
            disabled={!existing && !title.trim()}
          >
            {t('sources.addCitation')}
          </button>
        </div>
      </section>
    </div>
  )
}

function PersonMedia({ id, primary }: { id: string; primary: string | null }) {
  const { t } = useTranslation()
  const { go, setMediaFocus } = useApp()
  const { data: items = [] } = useQuery(q<MediaItem[]>('media.list', { target_type: 'person', target_id: id }))
  const input = useRef<HTMLInputElement>(null)
  return (
    <section className="card p-4" data-testid="person-media">
      <div className="mb-3 flex items-center gap-3">
        <h2 className="font-semibold">{t('nav.media')}</h2>
        <input
          ref={input}
          type="file"
          multiple
          className="sr-only"
          data-testid="person-media-input"
          aria-label={t('media.addFiles')}
          onChange={(e) =>
            e.target.files &&
            void uploadFiles([...e.target.files], { type: 'person', id }, t).then(() => act('project.status'))
          }
        />
        <button type="button" className="btn" onClick={() => input.current?.click()}>
          {t('media.addFiles')}
        </button>
      </div>
      {items.length === 0 && <p className="text-sm text-[var(--muted)]">{t('media.noneForPerson')}</p>}
      <ul className="flex flex-wrap gap-3">
        {items.map((m) => (
          <li key={m.id} className="w-28">
            <button
              type="button"
              className="block overflow-hidden rounded-lg border border-[var(--border)]"
              onClick={() => (setMediaFocus(m.id), go('media'))}
              aria-label={m.caption || m.name}
            >
              <MediaThumb item={m} size={110} />
            </button>
            <div className="truncate text-xs">{m.caption || m.name}</div>
            {primary === m.id ? (
              <span className="chip">{t('media.profile')}</span>
            ) : (
              m.kind === 'image' && (
                <button
                  type="button"
                  className="text-xs underline"
                  onClick={() => void act('media.set_primary', { person_id: id, media_id: m.id })}
                >
                  {t('media.makeProfile')}
                </button>
              )
            )}
          </li>
        ))}
      </ul>
    </section>
  )
}
