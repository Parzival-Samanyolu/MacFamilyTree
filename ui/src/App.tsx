import { QueryClientProvider } from '@tanstack/react-query'
import { useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { call } from './api/client'
import { CommandPalette, HelpDialog, Sidebar, StatusBar, Toasts, Toolbar } from './components/Shell'
import { Wizard } from './components/Wizard'
import { act, queryClient, refreshStatus } from './lib/query'
import { useApp, type View } from './store/app'
import { Dashboard } from './views/Dashboard'
import { FanChart } from './views/FanChart'
import { ImportExport } from './views/ImportExport'
import { Persons } from './views/Persons'
import { Quality } from './views/Quality'
import { Relationship } from './views/Relationship'
import { Settings } from './views/Settings'
import { Statistics } from './views/Statistics'
import { TreeView } from './views/TreeView'
import { CalendarView } from './views/Calendar'
import { Library } from './views/Library'
import { Reports } from './views/Reports'
import { Timeline } from './views/Timeline'

const VIEW_KEYS: View[] = [
  'dashboard',
  'persons',
  'tree',
  'fan',
  'timeline',
  'calendar',
  'relationship',
  'reports',
  'library',
]

function Router() {
  const view = useApp((s) => s.view)
  const open = useApp((s) => s.status.open)
  if (!open && view !== 'settings' && view !== 'importexport') return <Dashboard />
  switch (view) {
    case 'persons':
      return <Persons />
    case 'tree':
      return <TreeView />
    case 'fan':
      return <FanChart />
    case 'relationship':
      return <Relationship />
    case 'timeline':
      return <Timeline />
    case 'calendar':
      return <CalendarView />
    case 'reports':
      return <Reports />
    case 'library':
      return <Library />
    case 'quality':
      return <Quality />
    case 'statistics':
      return <Statistics />
    case 'importexport':
      return <ImportExport />
    case 'settings':
      return <Settings />
    default:
      return <Dashboard />
  }
}

function useGlobalShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey
      const target = e.target as HTMLElement
      const typing =
        target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.tagName === 'SELECT' ||
        target.isContentEditable
      const st = useApp.getState()
      if (mod && e.key.toLowerCase() === 'k') {
        e.preventDefault()
        st.setPalette(!st.paletteOpen)
      } else if (mod && e.key.toLowerCase() === 'z' && !typing) {
        e.preventDefault()
        void act(e.shiftKey ? 'history.redo' : 'history.undo')
      } else if (mod && e.key.toLowerCase() === 'y' && !typing) {
        e.preventDefault()
        void act('history.redo')
      } else if (mod && e.key.toLowerCase() === 'n') {
        e.preventDefault()
        if (st.status.open) {
          st.go('persons')
          setTimeout(() => window.dispatchEvent(new Event('kt:new-person')), 0)
        }
      } else if (mod && /^[1-9]$/.test(e.key)) {
        e.preventDefault()
        const v = VIEW_KEYS[Number(e.key) - 1]
        if (v && (st.status.open || v === 'dashboard' || v === 'settings' || v === 'importexport')) st.go(v)
      } else if (e.altKey && e.key === 'ArrowLeft') {
        e.preventDefault()
        st.goBack()
      } else if (e.altKey && e.key === 'ArrowRight') {
        e.preventDefault()
        st.goForward()
      } else if (e.key === '?' && !typing) st.setHelp(true)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])
}

function useAppearance() {
  const { theme, contrast, fontScale, lang } = useApp()
  const { i18n } = useTranslation()
  useEffect(() => {
    const mq = window.matchMedia?.('(prefers-color-scheme: dark)')
    const apply = () => {
      const dark = theme === 'dark' || (theme === 'system' && !!mq?.matches)
      document.documentElement.dataset.theme = dark ? 'dark' : 'light'
    }
    apply()
    mq?.addEventListener?.('change', apply)
    return () => mq?.removeEventListener?.('change', apply)
  }, [theme])
  useEffect(() => {
    document.documentElement.dataset.contrast = contrast ? 'high' : 'normal'
  }, [contrast])
  useEffect(() => {
    document.documentElement.style.setProperty('--font-scale', String(fontScale))
  }, [fontScale])
  useEffect(() => {
    void i18n.changeLanguage(lang)
    document.documentElement.lang = lang
  }, [lang, i18n])
  // let the Rust core format dates in the UI language
  useEffect(() => {
    if (useApp.getState().status.open)
      void call('settings.set', { key: 'lang', value: lang }).then(() => queryClient.invalidateQueries())
  }, [lang])
}

export function App() {
  const open = useApp((s) => s.status.open)
  useGlobalShortcuts()
  useAppearance()
  useEffect(() => {
    void refreshStatus().catch(() => undefined)
  }, [])
  // when a project opens (or the language changes) tell the core which language to use for dates
  const lang = useApp((s) => s.lang)
  useEffect(() => {
    if (open) void call('settings.set', { key: 'lang', value: lang }).then(() => queryClient.invalidateQueries())
  }, [open, lang])
  return (
    <div className="flex h-screen flex-col">
      <Toolbar />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <main className="min-w-0 flex-1 overflow-auto" id="main">
          <Router />
        </main>
      </div>
      <StatusBar />
      <CommandPalette />
      <HelpDialog />
      <Wizard />
      <Toasts />
    </div>
  )
}

export function Root() {
  return (
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  )
}
