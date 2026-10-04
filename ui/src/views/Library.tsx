import * as Tabs from '@radix-ui/react-tabs'
import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { Summary } from '../api/types'
import { Icons } from '../components/Icons'
import { Modal } from '../components/Modal'
import { PersonPicker } from '../components/PersonPicker'
import { act, q } from '../lib/query'
import { useApp } from '../store/app'

interface Source {
  id: string
  title: string
  author: string | null
  publication: string | null
  repository_id: string | null
  kind: string | null
  reliability: number | null
  text: string | null
  inline: number
}
interface Repo {
  id: string
  name: string
  address: string | null
  website: string | null
  notes: string | null
}
interface Task {
  id: string
  title: string
  description: string | null
  status: string
  priority: number
  due: number | null
  person?: Summary
}

const KINDS = ['census', 'birth', 'marriage', 'death', 'church', 'newspaper', 'website', 'book', 'interview']

function Field({ label, id, children }: { label: string; id: string; children: React.ReactNode }) {
  return (
    <div>
      <label className="label" htmlFor={id}>
        {label}
      </label>
      {children}
    </div>
  )
}

function SourceForm({ source, onClose }: { source: Partial<Source> | null; onClose: () => void }) {
  const { t } = useTranslation()
  const { data: repos } = useQuery(q<Repo[]>('rec.list', { table: 'repository' }))
  const [f, setF] = useState<Partial<Source>>(source ?? {})
  const set = (k: keyof Source, v: string | number | null) => setF({ ...f, [k]: v })
  const save = async () => {
    if (!f.title?.trim()) return
    const row: Record<string, unknown> = {
      id: f.id,
      title: f.title.trim(),
      author: f.author ?? '',
      publication: f.publication ?? '',
      repository_id: f.repository_id || null,
      kind: f.kind ?? null,
      reliability: f.reliability ?? null,
      text: f.text ?? '',
      inline: 0,
    }
    if (!f.id) delete row.id
    if (await act('rec.put', { table: 'source', row })) onClose()
  }
  const kind = f.kind ?? ''
  return (
    <form
      className="grid gap-3 sm:grid-cols-2"
      onSubmit={(e) => {
        e.preventDefault()
        void save()
      }}
    >
      <Field label={t('library.template')} id="src-kind">
        <select id="src-kind" className="input" value={kind} onChange={(e) => set('kind', e.target.value || null)}>
          <option value="">{t('library.tpl.none')}</option>
          {KINDS.map((k) => (
            <option key={k} value={k}>
              {t(`library.kind.${k}`)}
            </option>
          ))}
        </select>
      </Field>
      <Field label={t('library.reliability')} id="src-rel">
        <select
          id="src-rel"
          className="input"
          value={f.reliability ?? ''}
          onChange={(e) => set('reliability', e.target.value === '' ? null : Number(e.target.value))}
        >
          <option value="">–</option>
          {[0, 1, 2, 3].map((n) => (
            <option key={n} value={n}>
              {t(`library.rel.${n}`)}
            </option>
          ))}
        </select>
      </Field>
      <div className="sm:col-span-2">
        <Field label={t('sources.titleLabel')} id="src-title">
          <input
            id="src-title"
            className="input"
            value={f.title ?? ''}
            onChange={(e) => set('title', e.target.value)}
            placeholder={kind ? t(`library.hint.${kind}`) : ''}
            autoFocus
          />
        </Field>
      </div>
      <Field label={t('library.author')} id="src-author">
        <input
          id="src-author"
          className="input"
          value={f.author ?? ''}
          onChange={(e) => set('author', e.target.value)}
        />
      </Field>
      <Field label={t('library.publication')} id="src-pub">
        <input
          id="src-pub"
          className="input"
          value={f.publication ?? ''}
          onChange={(e) => set('publication', e.target.value)}
        />
      </Field>
      <Field label={t('library.repository')} id="src-repo">
        <select
          id="src-repo"
          className="input"
          value={f.repository_id ?? ''}
          onChange={(e) => set('repository_id', e.target.value || null)}
        >
          <option value="">–</option>
          {(repos ?? []).map((r) => (
            <option key={r.id} value={r.id}>
              {r.name}
            </option>
          ))}
        </select>
      </Field>
      <div className="sm:col-span-2">
        <Field label={t('library.text')} id="src-text">
          <textarea
            id="src-text"
            className="input min-h-20"
            value={f.text ?? ''}
            onChange={(e) => set('text', e.target.value)}
          />
        </Field>
      </div>
      <div className="flex justify-end gap-2 sm:col-span-2">
        <button type="button" className="btn" onClick={onClose}>
          {t('common.cancel')}
        </button>
        <button type="submit" className="btn btn-primary" disabled={!f.title?.trim()}>
          {t('common.save')}
        </button>
      </div>
    </form>
  )
}

function Sources() {
  const { t } = useTranslation()
  const { data: sources } = useQuery(q<Source[]>('rec.list', { table: 'source' }))
  const { data: cites } = useQuery(q<{ source_id: string }[]>('rec.list', { table: 'citation' }))
  const { data: repos } = useQuery(q<Repo[]>('rec.list', { table: 'repository' }))
  const [edit, setEdit] = useState<Partial<Source> | null>(null)
  const [filter, setFilter] = useState('')
  const uses = new Map<string, number>()
  for (const c of cites ?? []) uses.set(c.source_id, (uses.get(c.source_id) ?? 0) + 1)
  const rows = (sources ?? []).filter(
    (s) => !s.inline && `${s.title} ${s.author ?? ''}`.toLowerCase().includes(filter.toLowerCase()),
  )
  const repoName = (id: string | null) => (repos ?? []).find((r) => r.id === id)?.name ?? ''
  return (
    <div>
      <div className="mb-3 flex items-center gap-2">
        <input
          className="input"
          type="search"
          placeholder={t('library.filter')}
          aria-label={t('library.filter')}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
        <button type="button" className="btn btn-primary whitespace-nowrap" onClick={() => setEdit({})}>
          {Icons.plus} {t('library.addSource')}
        </button>
      </div>
      <div className="card overflow-hidden">
        <table className="w-full text-sm" data-testid="sources-table">
          <thead className="bg-[var(--surface-2)] text-left">
            <tr>
              <th className="px-3 py-2">{t('sources.titleLabel')}</th>
              <th className="px-3 py-2">{t('library.author')}</th>
              <th className="px-3 py-2">{t('library.repository')}</th>
              <th className="px-3 py-2">{t('library.citations')}</th>
              <th className="px-3 py-2">
                <span className="sr-only">{t('common.actions')}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {rows.length === 0 && (
              <tr>
                <td colSpan={5} className="px-3 py-4 text-[var(--muted)]">
                  {t('library.noSources')}
                </td>
              </tr>
            )}
            {rows.map((s) => (
              <tr key={s.id} className="border-t border-[var(--border)]">
                <td className="px-3 py-2">
                  <button
                    type="button"
                    className="font-medium text-[var(--accent)] hover:underline"
                    onClick={() => setEdit(s)}
                  >
                    {s.title}
                  </button>
                  {s.kind && <span className="chip ml-2">{t(`library.kind.${s.kind}`)}</span>}
                </td>
                <td className="px-3 py-2">{s.author}</td>
                <td className="px-3 py-2">{repoName(s.repository_id)}</td>
                <td className="px-3 py-2">{uses.get(s.id) ?? 0}</td>
                <td className="px-3 py-2 text-right">
                  <button
                    type="button"
                    className="btn btn-danger"
                    onClick={() =>
                      window.confirm(t('library.confirmDeleteSource', { title: s.title })) &&
                      act('rec.delete', { table: 'source', id: s.id })
                    }
                  >
                    {t('common.delete')}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <Modal
        open={edit !== null}
        onOpenChange={(o) => !o && setEdit(null)}
        title={edit?.id ? t('library.editSource') : t('library.addSource')}
        wide
      >
        {edit !== null && <SourceForm key={edit.id ?? 'new'} source={edit} onClose={() => setEdit(null)} />}
      </Modal>
    </div>
  )
}

function Repositories() {
  const { t } = useTranslation()
  const { data } = useQuery(q<Repo[]>('rec.list', { table: 'repository' }))
  const [edit, setEdit] = useState<Partial<Repo> | null>(null)
  const [f, setF] = useState<Partial<Repo>>({})
  const open = (r: Partial<Repo>) => {
    setF(r)
    setEdit(r)
  }
  const save = async () => {
    if (!f.name?.trim()) return
    const row: Record<string, unknown> = { ...f, name: f.name.trim() }
    if (await act('rec.put', { table: 'repository', row })) setEdit(null)
  }
  return (
    <div>
      <div className="mb-3 flex justify-end">
        <button type="button" className="btn btn-primary" onClick={() => open({})}>
          {Icons.plus} {t('library.addRepository')}
        </button>
      </div>
      <ul className="grid gap-3 sm:grid-cols-2" data-testid="repo-list">
        {(data ?? []).length === 0 && <li className="text-sm text-[var(--muted)]">{t('library.noRepositories')}</li>}
        {(data ?? []).map((r) => (
          <li key={r.id} className="card p-3">
            <button type="button" className="font-medium text-[var(--accent)] hover:underline" onClick={() => open(r)}>
              {r.name}
            </button>
            {r.address && <div className="text-sm text-[var(--muted)]">{r.address}</div>}
            {r.website && <div className="truncate text-sm">{r.website}</div>}
            <button
              type="button"
              className="btn btn-danger mt-2"
              onClick={() =>
                window.confirm(t('library.confirmDeleteRepo', { name: r.name })) &&
                act('rec.delete', { table: 'repository', id: r.id })
              }
            >
              {t('common.delete')}
            </button>
          </li>
        ))}
      </ul>
      <Modal
        open={edit !== null}
        onOpenChange={(o) => !o && setEdit(null)}
        title={f.id ? t('library.editRepository') : t('library.addRepository')}
      >
        <form
          className="grid gap-3"
          onSubmit={(e) => {
            e.preventDefault()
            void save()
          }}
        >
          <Field label={t('library.name')} id="rp-name">
            <input
              id="rp-name"
              className="input"
              value={f.name ?? ''}
              onChange={(e) => setF({ ...f, name: e.target.value })}
              autoFocus
            />
          </Field>
          <Field label={t('library.address')} id="rp-addr">
            <textarea
              id="rp-addr"
              className="input"
              value={f.address ?? ''}
              onChange={(e) => setF({ ...f, address: e.target.value })}
            />
          </Field>
          <Field label={t('library.website')} id="rp-web">
            <input
              id="rp-web"
              className="input"
              value={f.website ?? ''}
              onChange={(e) => setF({ ...f, website: e.target.value })}
            />
          </Field>
          <div className="flex justify-end gap-2">
            <button type="button" className="btn" onClick={() => setEdit(null)}>
              {t('common.cancel')}
            </button>
            <button type="submit" className="btn btn-primary" disabled={!f.name?.trim()}>
              {t('common.save')}
            </button>
          </div>
        </form>
      </Modal>
    </div>
  )
}

const STATUSES = ['open', 'doing', 'done']

function toDate(secs: number | null) {
  return secs ? new Date(secs * 1000).toISOString().slice(0, 10) : ''
}

function Tasks() {
  const { t } = useTranslation()
  const { personId, select, go } = useApp()
  const { data } = useQuery(q<Task[]>('task.list'))
  const [title, setTitle] = useState('')
  const [priority, setPriority] = useState(1)
  const [due, setDue] = useState('')
  const [link, setLink] = useState<Summary | null>(null)
  const [show, setShow] = useState<'active' | 'all'>('active')
  const add = async () => {
    if (!title.trim()) return
    const r = await act('task.save', {
      title,
      priority,
      status: 'open',
      due: due ? Math.floor(new Date(due).getTime() / 1000) : null,
      person_id: link?.id,
    })
    if (r) {
      setTitle('')
      setDue('')
      setLink(null)
    }
  }
  const list = (data ?? []).filter((x) => show === 'all' || x.status !== 'done')
  return (
    <div>
      <form
        className="card mb-4 grid gap-3 p-3 sm:grid-cols-[1fr_8rem_9rem]"
        onSubmit={(e) => {
          e.preventDefault()
          void add()
        }}
      >
        <Field label={t('library.taskTitle')} id="tk-title">
          <input id="tk-title" className="input" value={title} onChange={(e) => setTitle(e.target.value)} />
        </Field>
        <Field label={t('library.priority')} id="tk-pri">
          <select id="tk-pri" className="input" value={priority} onChange={(e) => setPriority(Number(e.target.value))}>
            {[0, 1, 2, 3].map((n) => (
              <option key={n} value={n}>
                {t(`library.prio.${n}`)}
              </option>
            ))}
          </select>
        </Field>
        <Field label={t('library.due')} id="tk-due">
          <input id="tk-due" type="date" className="input" value={due} onChange={(e) => setDue(e.target.value)} />
        </Field>
        <div className="sm:col-span-3">
          <PersonPicker label={t('library.linkPerson')} value={link} onChange={setLink} />
          {personId && !link && <p className="mt-1 text-xs text-[var(--muted)]">{t('library.linkHint')}</p>}
        </div>
        <div className="sm:col-span-3">
          <button type="submit" className="btn btn-primary" disabled={!title.trim()}>
            {Icons.plus} {t('library.addTask')}
          </button>
        </div>
      </form>
      <label className="mb-2 flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={show === 'all'}
          onChange={(e) => setShow(e.target.checked ? 'all' : 'active')}
        />{' '}
        {t('library.showDone')}
      </label>
      <ul className="flex flex-col gap-2" data-testid="task-list">
        {list.length === 0 && <li className="text-sm text-[var(--muted)]">{t('library.noTasks')}</li>}
        {list.map((x) => (
          <li key={x.id} className="card flex flex-wrap items-center gap-3 p-3" data-status={x.status}>
            <select
              className="input !w-auto"
              value={x.status}
              aria-label={t('library.status')}
              onChange={(e) => act('task.save', { id: x.id, title: x.title, status: e.target.value })}
            >
              {STATUSES.map((s) => (
                <option key={s} value={s}>
                  {t(`library.st.${s}`)}
                </option>
              ))}
            </select>
            <span className={`min-w-0 flex-1 ${x.status === 'done' ? 'line-through opacity-60' : ''}`}>{x.title}</span>
            <span className="chip">{t(`library.prio.${x.priority}`)}</span>
            {x.due && <span className="text-xs text-[var(--muted)]">{toDate(x.due)}</span>}
            {x.person && (
              <button
                type="button"
                className="btn"
                onClick={() => {
                  select(x.person!.id)
                  go('persons', x.person!.id)
                }}
              >
                {x.person.name}
              </button>
            )}
            <button
              type="button"
              className="btn btn-danger"
              onClick={() => act('rec.delete', { table: 'task', id: x.id })}
            >
              {t('common.delete')}
            </button>
          </li>
        ))}
      </ul>
    </div>
  )
}

interface Unsourced {
  total: number
  items: { event_id: string; kind: string; label: string; person_id: string | null; date_text: string | null }[]
}

function UnsourcedFacts() {
  const { t } = useTranslation()
  const { select, go } = useApp()
  const { data } = useQuery(q<Unsourced>('sources.unsourced', { limit: 200 }))
  return (
    <div>
      <p className="mb-3 text-sm text-[var(--muted)]">{t('library.unsourcedHint', { count: data?.total ?? 0 })}</p>
      <ul className="flex flex-col gap-1" data-testid="unsourced-list">
        {(data?.items ?? []).map((x) => (
          <li key={x.event_id} className="flex items-center gap-2 text-sm">
            <span className="chip">{t(`event.kind.${x.kind}`)}</span>
            <button
              type="button"
              className="text-left hover:underline"
              onClick={() => {
                if (x.person_id) {
                  select(x.person_id)
                  go('persons', x.person_id)
                }
              }}
            >
              {x.label}
            </button>
            <span className="text-[var(--muted)]">{x.date_text}</span>
          </li>
        ))}
        {data?.total === 0 && <li className="text-[var(--muted)]">{t('library.allSourced')}</li>}
      </ul>
    </div>
  )
}

export function Library() {
  const { t } = useTranslation()
  return (
    <div className="mx-auto max-w-5xl p-6">
      <h1 className="mb-4 text-2xl font-semibold tracking-tight">{t('nav.library')}</h1>
      <Tabs.Root defaultValue="sources">
        <Tabs.List className="mb-4 flex gap-1 border-b border-[var(--border)]">
          {['sources', 'repositories', 'tasks', 'unsourced'].map((k) => (
            <Tabs.Trigger
              key={k}
              value={k}
              className="px-3 py-2 text-sm data-[state=active]:border-b-2 data-[state=active]:border-[var(--accent)] data-[state=active]:font-medium data-[state=active]:text-[var(--accent)]"
            >
              {t(`library.tab.${k}`)}
            </Tabs.Trigger>
          ))}
        </Tabs.List>
        <Tabs.Content value="sources">
          <Sources />
        </Tabs.Content>
        <Tabs.Content value="repositories">
          <Repositories />
        </Tabs.Content>
        <Tabs.Content value="tasks">
          <Tasks />
        </Tabs.Content>
        <Tabs.Content value="unsourced">
          <UnsourcedFacts />
        </Tabs.Content>
      </Tabs.Root>
    </div>
  )
}
