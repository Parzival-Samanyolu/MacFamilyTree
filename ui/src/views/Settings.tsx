import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { act, q } from '../lib/query'
import { useApp } from '../store/app'

export function Settings() {
  const { t } = useTranslation()
  const { theme, setTheme, contrast, setContrast, fontScale, setFontScale, lang, setLang, status } = useApp()
  const { data: prefs } = useQuery({ ...q<Record<string, string>>('settings.get'), enabled: status.open })
  const setPref = (key: string, value: string) => void act('settings.set', { key, value })
  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-4 p-6">
      <h1 className="text-2xl font-semibold tracking-tight">{t('nav.settings')}</h1>
      <section className="card grid gap-4 p-4 sm:grid-cols-2">
        <div>
          <label className="label" htmlFor="s-lang">
            {t('settings.language')}
          </label>
          <select
            id="s-lang"
            className="input"
            value={lang}
            onChange={(e) => {
              const l = e.target.value as 'en' | 'tr'
              setLang(l)
              if (status.open) setPref('lang', l)
            }}
          >
            <option value="en">English</option>
            <option value="tr">Türkçe</option>
          </select>
        </div>
        <div>
          <label className="label" htmlFor="s-theme">
            {t('settings.theme')}
          </label>
          <select
            id="s-theme"
            className="input"
            value={theme}
            onChange={(e) => setTheme(e.target.value as typeof theme)}
          >
            <option value="system">{t('settings.themeSystem')}</option>
            <option value="light">{t('settings.themeLight')}</option>
            <option value="dark">{t('settings.themeDark')}</option>
          </select>
        </div>
        <div>
          <label className="label" htmlFor="s-font">
            {t('settings.fontSize', { pct: Math.round(fontScale * 100) })}
          </label>
          <input
            id="s-font"
            type="range"
            min="0.85"
            max="1.5"
            step="0.05"
            value={fontScale}
            onChange={(e) => setFontScale(Number(e.target.value))}
            className="w-full"
          />
        </div>
        <label className="flex items-center gap-2 self-end text-sm">
          <input type="checkbox" checked={contrast} onChange={(e) => setContrast(e.target.checked)} />
          {t('settings.contrast')}
        </label>
      </section>
      <section className="card grid gap-4 p-4 sm:grid-cols-2">
        <div className="sm:col-span-2 text-sm text-[var(--muted)]">
          {status.open ? t('settings.projectHint') : t('settings.noProject')}
        </div>
        <div>
          <label className="label" htmlFor="s-culture">
            {t('settings.culture')}
          </label>
          <select
            id="s-culture"
            className="input"
            disabled={!status.open}
            value={prefs?.naming_culture ?? 'patrilineal'}
            onChange={(e) => setPref('naming_culture', e.target.value)}
          >
            <option value="patrilineal">{t('culture.patrilineal')}</option>
            <option value="turkish">{t('culture.turkish')}</option>
            <option value="spanish">{t('culture.spanish')}</option>
            <option value="patronymic">{t('culture.patronymic')}</option>
            <option value="slavic">{t('culture.slavic')}</option>
          </select>
        </div>
      </section>
      <p className="text-xs text-[var(--muted)]">{t('settings.privacy')}</p>
    </div>
  )
}
