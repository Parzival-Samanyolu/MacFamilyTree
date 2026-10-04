import { create } from 'zustand'
import type { Status } from '../api/types'

export type View =
  | 'dashboard'
  | 'persons'
  | 'tree'
  | 'fan'
  | 'timeline'
  | 'map'
  | 'media'
  | 'calendar'
  | 'relationship'
  | 'reports'
  | 'library'
  | 'quality'
  | 'statistics'
  | 'importexport'
  | 'settings'

export type Theme = 'system' | 'light' | 'dark'

interface Toast {
  id: number
  text: string
  kind: 'info' | 'error'
  action?: { label: string; run: () => void }
}

interface Nav {
  view: View
  personId: string | null
}

interface AppState {
  status: Status
  view: View
  personId: string | null
  back: Nav[]
  forward: Nav[]
  theme: Theme
  contrast: boolean
  fontScale: number
  lang: 'en' | 'tr'
  sidebarCollapsed: boolean
  paletteOpen: boolean
  helpOpen: boolean
  wizardOpen: boolean
  toasts: Toast[]
  mediaFocus: string | null
  setMediaFocus: (id: string | null) => void
  setStatus: (s: Status) => void
  go: (view: View, personId?: string | null) => void
  select: (personId: string | null) => void
  goBack: () => void
  goForward: () => void
  setTheme: (t: Theme) => void
  setContrast: (b: boolean) => void
  setFontScale: (n: number) => void
  setLang: (l: 'en' | 'tr') => void
  toggleSidebar: () => void
  setPalette: (b: boolean) => void
  setHelp: (b: boolean) => void
  setWizard: (b: boolean) => void
  toast: (text: string, kind?: 'info' | 'error', action?: Toast['action']) => void
  dismissToast: (id: number) => void
}

function load<T>(key: string, fallback: T): T {
  try {
    const v = localStorage.getItem(key)
    return v === null ? fallback : (JSON.parse(v) as T)
  } catch {
    return fallback
  }
}
function save(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {
    /* storage unavailable: preferences simply do not persist */
  }
}

let toastId = 1

export const useApp = create<AppState>((set, get) => ({
  status: { open: false },
  view: 'dashboard',
  personId: null,
  back: [],
  forward: [],
  theme: load<Theme>('kt.theme', 'system'),
  contrast: load('kt.contrast', false),
  fontScale: load('kt.fontScale', 1),
  lang: load<'en' | 'tr'>('kt.lang', navigator.language?.startsWith('tr') ? 'tr' : 'en'),
  sidebarCollapsed: load('kt.sidebar', false),
  paletteOpen: false,
  helpOpen: false,
  wizardOpen: false,
  toasts: [],
  mediaFocus: null,
  setMediaFocus: (mediaFocus) => set({ mediaFocus }),
  setStatus: (status) => set({ status }),
  go: (view, personId) => {
    const s = get()
    const next: Nav = { view, personId: personId === undefined ? s.personId : personId }
    if (next.view === s.view && next.personId === s.personId) return
    set({
      back: [...s.back, { view: s.view, personId: s.personId }].slice(-100),
      forward: [],
      view: next.view,
      personId: next.personId,
    })
  },
  select: (personId) => {
    const s = get()
    if (personId === s.personId) return
    set({ back: [...s.back, { view: s.view, personId: s.personId }].slice(-100), forward: [], personId })
  },
  goBack: () => {
    const s = get()
    const prev = s.back[s.back.length - 1]
    if (!prev) return
    set({
      back: s.back.slice(0, -1),
      forward: [{ view: s.view, personId: s.personId }, ...s.forward],
      view: prev.view,
      personId: prev.personId,
    })
  },
  goForward: () => {
    const s = get()
    const next = s.forward[0]
    if (!next) return
    set({
      forward: s.forward.slice(1),
      back: [...s.back, { view: s.view, personId: s.personId }],
      view: next.view,
      personId: next.personId,
    })
  },
  setTheme: (theme) => {
    save('kt.theme', theme)
    set({ theme })
  },
  setContrast: (contrast) => {
    save('kt.contrast', contrast)
    set({ contrast })
  },
  setFontScale: (fontScale) => {
    save('kt.fontScale', fontScale)
    set({ fontScale })
  },
  setLang: (lang) => {
    save('kt.lang', lang)
    set({ lang })
  },
  toggleSidebar: () => {
    const v = !get().sidebarCollapsed
    save('kt.sidebar', v)
    set({ sidebarCollapsed: v })
  },
  setPalette: (paletteOpen) => set({ paletteOpen }),
  setHelp: (helpOpen) => set({ helpOpen }),
  setWizard: (wizardOpen) => set({ wizardOpen }),
  toast: (text, kind = 'info', action) => {
    const id = toastId++
    set((s) => ({ toasts: [...s.toasts, { id, text, kind, action }].slice(-4) }))
    setTimeout(() => get().dismissToast(id), action ? 8000 : 4500)
  },
  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}))
