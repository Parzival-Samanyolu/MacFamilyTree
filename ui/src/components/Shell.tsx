import { useQuery } from '@tanstack/react-query'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { Summary } from '../api/types'
import { act, q } from '../lib/query'
import { useApp, type View } from '../store/app'
import { Icons } from './Icons'
import { Modal } from './Modal'
import { PersonChip } from './PersonChip'

const NAV: { view: View; icon: keyof typeof Icons; key: string }[] = [
  { view: 'dashboard', icon: 'home', key: 'nav.dashboard' },
  { view: 'persons', icon: 'people', key: 'nav.persons' },
  { view: 'tree', icon: 'tree', key: 'nav.tree' },
  { view: 'fan', icon: 'fan', key: 'nav.fan' },
  { view: 'relationship', icon: 'link', key: 'nav.relationship' },
  { view: 'quality', icon: 'check', key: 'nav.quality' },
  { view: 'statistics', icon: 'chart', key: 'nav.statistics' },
  { view: 'importexport', icon: 'file', key: 'nav.importexport' },
  { view: 'settings', icon: 'gear', key: 'nav.settings' },
]

export function Sidebar() {
  const { t } = useTranslation()
  const { view, go, sidebarCollapsed, toggleSidebar, status } = useApp()
  return (
    <nav
      aria-label={t('nav.main')}
      className="flex h-full flex-col border-r border-[var(--border)] bg-[var(--surface)] p-2"
      style={{ width: sidebarCollapsed ? 56 : 208, transition: 'width 200ms ease' }}
    >
      <button
        type="button"
        className="btn mb-2 justify-center"
        onClick={toggleSidebar}
        aria-label={t('nav.toggle')}
        aria-expanded={!sidebarCollapsed}
      >
        {Icons.menu}
      </button>
      <ul className="flex flex-col gap-1">
        {NAV.map((n, i) => {
          const disabled = !status.open && n.view !== 'dashboard' && n.view !== 'settings' && n.view !== 'importexport'
          return (
            <li key={n.view}>
              <button
                type="button"
                disabled={disabled}
                title={`${t(n.key)}  (${navigator.platform.includes('Mac') ? '⌘' : 'Ctrl+'}${i + 1})`}
                aria-current={view === n.view ? 'page' : undefined}
                onClick={() => go(n.view)}
                className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm disabled:opacity-40 ${view === n.view ? 'bg-[var(--accent-soft)] font-medium text-[var(--accent)]' : 'hover:bg-[var(--surface-2)]'}`}
              >
                {Icons[n.icon]}
                {!sidebarCollapsed && <span>{t(n.key)}</span>}
              </button>
            </li>
          )
        })}
      </ul>
    </nav>
  )
}

export function Toolbar() {
  const { t } = useTranslation()
  const { status, back, forward, goBack, goForward, setPalette, setHelp } = useApp()
  const undo = async () => {
    const r = await act<{ label: string | null }>('history.undo')
    if (r?.label)
      useApp.getState().toast(t('toast.undone', { label: r.label }), 'info', {
        label: t('action.redo'),
        run: () => void act('history.redo'),
      })
  }
  const redo = () => void act('history.redo')
  return (
    <header className="flex items-center gap-2 border-b border-[var(--border)] bg-[var(--surface)] px-3 py-2">
      <span className="mr-2 font-semibold tracking-tight text-[var(--accent)]">KinTree</span>
      <button
        type="button"
        className="btn"
        onClick={goBack}
        disabled={!back.length}
        aria-label={t('action.back')}
        title="Alt+←"
      >
        {Icons.back}
      </button>
      <button
        type="button"
        className="btn"
        onClick={goForward}
        disabled={!forward.length}
        aria-label={t('action.forward')}
        title="Alt+→"
      >
        {Icons.forward}
      </button>
      <span className="mx-1 h-5 w-px bg-[var(--border)]" />
      <button
        type="button"
        className="btn"
        onClick={undo}
        disabled={!status.can_undo}
        aria-label={t('action.undo')}
        title={status.undo_label ? `${t('action.undo')}: ${status.undo_label}` : t('action.undo')}
      >
        {Icons.undo}
      </button>
      <button type="button" className="btn" onClick={redo} disabled={!status.can_redo} aria-label={t('action.redo')}>
        {Icons.redo}
      </button>
      <div className="flex-1" />
      <button
        type="button"
        className="btn min-w-48 justify-between"
        onClick={() => setPalette(true)}
        aria-label={t('palette.open')}
      >
        <span className="flex items-center gap-2 text-[var(--muted)]">
          {Icons.search} {t('palette.placeholder')}
        </span>
        <kbd className="text-xs text-[var(--muted)]">{navigator.platform.includes('Mac') ? '⌘K' : 'Ctrl+K'}</kbd>
      </button>
      <button type="button" className="btn" onClick={() => setHelp(true)} aria-label={t('help.title')}>
        {Icons.help}
      </button>
    </header>
  )
}

export function StatusBar() {
  const { t } = useTranslation()
  const { status } = useApp()
  return (
    <footer className="border-t border-[var(--border)] bg-[var(--surface)] px-3 py-1 text-xs text-[var(--muted)]">
      <div className="flex items-center gap-4" role="status">
        {status.open ? (
          <>
            <span>{t('status.persons', { count: status.persons ?? 0 })}</span>
            <span>{t('status.families', { count: status.families ?? 0 })}</span>
            <span className="truncate">{status.path ?? t('status.unsaved')}</span>
            <span className="flex-1" />
            {status.undo_label && <span>{t('status.lastAction', { label: status.undo_label })}</span>}
          </>
        ) : (
          <span>{t('status.noProject')}</span>
        )}
      </div>
    </footer>
  )
}

export function Toasts() {
  const { toasts, dismissToast } = useApp()
  return (
    <div className="pointer-events-none fixed bottom-10 right-4 z-[60] flex flex-col gap-2" aria-live="polite">
      {toasts.map((x) => (
        <div
          key={x.id}
          className={`card pointer-events-auto flex items-center gap-3 px-4 py-2 text-sm ${x.kind === 'error' ? 'border-[var(--danger)]' : ''}`}
          role={x.kind === 'error' ? 'alert' : 'status'}
        >
          <span className={x.kind === 'error' ? 'text-[var(--danger)]' : ''}>{x.text}</span>
          {x.action && (
            <button
              type="button"
              className="btn"
              onClick={() => {
                x.action?.run()
                dismissToast(x.id)
              }}
            >
              {x.action.label}
            </button>
          )}
        </div>
      ))}
    </div>
  )
}

interface Cmd {
  id: string
  label: string
  run: () => void
}

export function CommandPalette() {
  const { t } = useTranslation()
  const { paletteOpen, setPalette } = useApp()
  return (
    <Modal open={paletteOpen} onOpenChange={setPalette} title={t('palette.title')}>
      {/* mounted only while open, so the query and highlighted row reset every time */}
      <PaletteBody />
    </Modal>
  )
}

function PaletteBody() {
  const { t } = useTranslation()
  const { paletteOpen, setPalette, go, select, status, setWizard } = useApp()
  const [text, setText] = useState('')
  const [index, setIndex] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const { data: people } = useQuery({
    ...q<Summary[]>('search', { q: text, limit: 8 }),
    enabled: paletteOpen && status.open && text.trim().length > 0,
  })
  const commands: Cmd[] = useMemo(
    () => [
      ...(
        [
          'dashboard',
          'persons',
          'tree',
          'fan',
          'relationship',
          'quality',
          'statistics',
          'importexport',
          'settings',
        ] as View[]
      ).map((v) => ({ id: `go-${v}`, label: `${t('palette.go')} ${t(`nav.${v}`)}`, run: () => go(v) })),
      { id: 'undo', label: t('action.undo'), run: () => void act('history.undo') },
      { id: 'redo', label: t('action.redo'), run: () => void act('history.redo') },
      { id: 'wizard', label: t('wizard.title'), run: () => setWizard(true) },
    ],
    [t, go, setWizard],
  )
  const filtered = commands
    .filter((c) => !text.trim() || c.label.toLowerCase().includes(text.trim().toLowerCase()))
    .slice(0, 8)
  const items = [
    ...(people ?? []).map((p) => ({ kind: 'person' as const, p })),
    ...filtered.map((c) => ({ kind: 'cmd' as const, c })),
  ]
  useEffect(() => {
    const h = setTimeout(() => inputRef.current?.focus(), 30)
    return () => clearTimeout(h)
  }, [])
  const choose = (i: number) => {
    const it = items[i]
    if (!it) return
    setPalette(false)
    if (it.kind === 'person') {
      select(it.p.id)
      go('persons', it.p.id)
    } else it.c.run()
  }
  return (
    <>
      <input
        ref={inputRef}
        className="input"
        value={text}
        placeholder={t('palette.placeholder')}
        aria-label={t('palette.placeholder')}
        role="combobox"
        aria-expanded="true"
        aria-controls="palette-list"
        onChange={(e) => {
          setText(e.target.value)
          setIndex(0)
        }}
        onKeyDown={(e) => {
          if (e.key === 'ArrowDown') {
            e.preventDefault()
            setIndex((i) => Math.min(i + 1, items.length - 1))
          } else if (e.key === 'ArrowUp') {
            e.preventDefault()
            setIndex((i) => Math.max(i - 1, 0))
          } else if (e.key === 'Enter') choose(index)
        }}
      />
      <ul id="palette-list" role="listbox" className="mt-2 flex max-h-80 flex-col gap-1 overflow-auto">
        {items.length === 0 && <li className="px-2 py-1 text-sm text-[var(--muted)]">{t('common.noResults')}</li>}
        {items.map((it, i) => (
          <li key={it.kind === 'person' ? it.p.id : it.c.id} role="option" aria-selected={i === index}>
            <button
              type="button"
              className={`flex w-full items-center rounded-md px-2 py-1.5 text-left text-sm ${i === index ? 'bg-[var(--accent-soft)]' : 'hover:bg-[var(--surface-2)]'}`}
              onMouseEnter={() => setIndex(i)}
              onClick={() => choose(i)}
            >
              {it.kind === 'person' ? <PersonChip s={it.p} /> : it.c.label}
            </button>
          </li>
        ))}
      </ul>
    </>
  )
}

export function HelpDialog() {
  const { t } = useTranslation()
  const { helpOpen, setHelp } = useApp()
  const mod = navigator.platform.includes('Mac') ? '⌘' : 'Ctrl+'
  const rows: [string, string][] = [
    [`${mod}K`, t('help.palette')],
    [`${mod}Z / ${mod}⇧Z`, t('help.undoRedo')],
    [`${mod}1…9`, t('help.views')],
    [`${mod}N`, t('help.newPerson')],
    ['Alt+← / Alt+→', t('help.history')],
    ['← ↑ → ↓', t('help.treeKeys')],
    ['Enter / E', t('help.edit')],
    ['+ / − / 0', t('help.zoom')],
  ]
  return (
    <Modal open={helpOpen} onOpenChange={setHelp} title={t('help.title')}>
      <p className="mb-3 text-sm text-[var(--muted)]">{t('help.intro')}</p>
      <table className="w-full text-sm">
        <tbody>
          {rows.map(([k, v]) => (
            <tr key={k} className="border-t border-[var(--border)]">
              <td className="py-1.5 pr-4 font-mono text-xs">{k}</td>
              <td className="py-1.5">{v}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Modal>
  )
}
