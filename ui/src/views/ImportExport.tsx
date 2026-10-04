import { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { base64ToBytes, bytesToBase64, call } from '../api/client'
import type { ImportReport } from '../api/types'
import { downloadBlob } from '../lib/format'
import { act } from '../lib/query'
import { useApp } from '../store/app'

function OtherFormats() {
  const { t } = useTranslation()
  const { status, toast, go } = useApp()
  const csvRef = useRef<HTMLInputElement>(null)
  const [warnings, setWarnings] = useState<string[] | null>(null)
  const download = async (cmd: string, name: string, type: string) => {
    try {
      const r = await call<{ data: string; size: number }>(cmd, {})
      downloadBlob(name, base64ToBytes(r.data) as BlobPart, type)
      toast(t('export.done', { size: Math.max(1, Math.round(r.size / 1024)) }))
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  const importCsv = async (f: File | undefined) => {
    if (!f) return
    const bytes = new Uint8Array(await f.arrayBuffer())
    const r = await act<{ persons: number; families: number; warnings: string[] }>('csv.import', {
      data: bytesToBase64(bytes),
    })
    if (r) {
      setWarnings(r.warnings)
      toast(t('csv.imported', { persons: r.persons, families: r.families }))
      if (!r.warnings.length) go('persons')
    }
    if (csvRef.current) csvRef.current.value = ''
  }
  return (
    <section className="card p-4 lg:col-span-2">
      <h2 className="mb-2 text-lg font-semibold">{t('csv.title')}</h2>
      <p className="mb-3 text-sm text-[var(--muted)]">{t('csv.hint')}</p>
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          className="btn"
          disabled={!status.open}
          onClick={() => download('export.csv', 'people.csv', 'text/csv')}
        >
          {t('csv.export')}
        </button>
        <button
          type="button"
          className="btn"
          disabled={!status.open}
          onClick={() => download('export.json', 'family-tree.json', 'application/json')}
        >
          {t('csv.exportJson')}
        </button>
        <button
          type="button"
          className="btn"
          disabled={!status.open}
          onClick={() => download('export.ical', 'family-calendar.ics', 'text/calendar')}
        >
          {t('csv.exportIcs')}
        </button>
        <label className="btn cursor-pointer" htmlFor="csv-file">
          {t('csv.import')}
        </label>
        <input
          id="csv-file"
          ref={csvRef}
          type="file"
          accept=".csv,text/csv"
          className="sr-only"
          data-testid="csv-file"
          onChange={(e) => importCsv(e.target.files?.[0])}
        />
      </div>
      {warnings && warnings.length > 0 && (
        <ul className="mt-3 list-disc pl-5 text-sm text-[var(--warn)]" data-testid="csv-warnings">
          {warnings.map((w, i) => (
            <li key={i}>{w}</li>
          ))}
        </ul>
      )}
    </section>
  )
}

function Packages() {
  const { t } = useTranslation()
  const { status, toast, go } = useApp()
  const [site, setSite] = useState({ title: '', lang: 'en', privacy: 'exclude' })
  const [pw, setPw] = useState('')
  const [pw2, setPw2] = useState('')
  const encRef = useRef<HTMLInputElement>(null)
  const grab = async (cmd: string, args: Record<string, unknown>, name: string, type: string) => {
    try {
      const r = await call<{ data: string; size: number }>(cmd, args)
      downloadBlob(name, base64ToBytes(r.data) as BlobPart, type)
      toast(t('export.done', { size: Math.max(1, Math.round(r.size / 1024)) }))
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  const openEncrypted = async (f: File | undefined) => {
    if (!f) return
    const r = await act('project.import_encrypted', {
      data: bytesToBase64(new Uint8Array(await f.arrayBuffer())),
      password: pw,
    })
    if (r) {
      toast(t('packages.opened'))
      go('dashboard')
    }
    if (encRef.current) encRef.current.value = ''
  }
  const mismatch = pw !== '' && pw2 !== '' && pw !== pw2
  return (
    <section className="card p-4 lg:col-span-2" data-testid="packages">
      <h2 className="mb-2 text-lg font-semibold">{t('packages.title')}</h2>
      <div className="grid gap-4 md:grid-cols-2">
        <div>
          <h3 className="mb-1 font-medium">{t('packages.website')}</h3>
          <p className="mb-2 text-sm text-[var(--muted)]">{t('packages.websiteHint')}</p>
          <label className="label" htmlFor="site-title">
            {t('packages.siteTitle')}
          </label>
          <input
            id="site-title"
            className="input mb-2"
            value={site.title}
            onChange={(e) => setSite({ ...site, title: e.target.value })}
          />
          <div className="mb-2 grid grid-cols-2 gap-2">
            <div>
              <label className="label" htmlFor="site-lang">
                {t('reports.language')}
              </label>
              <select
                id="site-lang"
                className="input"
                value={site.lang}
                onChange={(e) => setSite({ ...site, lang: e.target.value })}
              >
                <option value="en">English</option>
                <option value="tr">Türkçe</option>
              </select>
            </div>
            <div>
              <label className="label" htmlFor="site-priv">
                {t('reports.privacy')}
              </label>
              <select
                id="site-priv"
                className="input"
                value={site.privacy}
                onChange={(e) => setSite({ ...site, privacy: e.target.value })}
              >
                <option value="off">{t('reports.privacyOff')}</option>
                <option value="mask">{t('export.livingMask')}</option>
                <option value="exclude">{t('export.livingExclude')}</option>
              </select>
            </div>
          </div>
          <button
            type="button"
            className="btn btn-primary"
            disabled={!status.open}
            onClick={() =>
              grab(
                'export.website',
                { ...site, title: site.title || undefined },
                'family-website.zip',
                'application/zip',
              )
            }
          >
            {t('packages.websiteButton')}
          </button>
        </div>
        <div>
          <h3 className="mb-1 font-medium">{t('packages.gedzip')}</h3>
          <p className="mb-2 text-sm text-[var(--muted)]">{t('packages.gedzipHint')}</p>
          <button
            type="button"
            className="btn"
            disabled={!status.open}
            onClick={() => grab('export.gedzip', {}, 'family-tree.gdz', 'application/zip')}
          >
            {t('packages.gedzipButton')}
          </button>
        </div>
        <div className="md:col-span-2">
          <h3 className="mb-1 font-medium">{t('packages.encrypted')}</h3>
          <p className="mb-2 text-sm text-[var(--muted)]">{t('packages.encryptedHint')}</p>
          <div className="grid gap-2 sm:grid-cols-2">
            <div>
              <label className="label" htmlFor="enc-pw">
                {t('packages.password')}
              </label>
              <input
                id="enc-pw"
                type="password"
                autoComplete="new-password"
                className="input"
                value={pw}
                onChange={(e) => setPw(e.target.value)}
              />
            </div>
            <div>
              <label className="label" htmlFor="enc-pw2">
                {t('packages.passwordAgain')}
              </label>
              <input
                id="enc-pw2"
                type="password"
                autoComplete="new-password"
                className="input"
                value={pw2}
                onChange={(e) => setPw2(e.target.value)}
              />
            </div>
          </div>
          {mismatch && (
            <p role="alert" className="mt-1 text-sm text-[var(--danger)]">
              {t('packages.mismatch')}
            </p>
          )}
          <div className="mt-2 flex flex-wrap items-center gap-2">
            <button
              type="button"
              className="btn"
              disabled={!status.open || !pw || pw !== pw2}
              onClick={() =>
                grab('project.export_encrypted', { password: pw }, 'family-tree.ktenc', 'application/octet-stream')
              }
            >
              {t('packages.encryptButton')}
            </button>
            <label className={`btn ${pw ? 'cursor-pointer' : 'opacity-50'}`} htmlFor="enc-file">
              {t('packages.openButton')}
            </label>
            <input
              id="enc-file"
              ref={encRef}
              type="file"
              accept=".ktenc"
              className="sr-only"
              disabled={!pw}
              data-testid="enc-file"
              onChange={(e) => openEncrypted(e.target.files?.[0])}
            />
          </div>
          <p className="mt-2 text-xs text-[var(--muted)]">{t('packages.warning')}</p>
        </div>
      </div>
    </section>
  )
}

function ProjectFiles() {
  const { t } = useTranslation()
  const { status, toast, go } = useApp()
  const [path, setPath] = useState('')
  const run = async (cmd: string, label: string) => {
    if (!path.trim()) return
    const r = await act(cmd, { path: path.trim() })
    if (r) {
      toast(label)
      if (cmd !== 'project.backup') go('dashboard')
    }
  }
  return (
    <section className="card p-4 lg:col-span-2">
      <h2 className="mb-2 text-lg font-semibold">{t('project.title')}</h2>
      <p className="mb-3 text-sm text-[var(--muted)]">{t('project.hint')}</p>
      <div className="flex flex-wrap items-end gap-2">
        <div className="min-w-64 flex-1">
          <label className="label" htmlFor="proj-path">
            {t('project.path')}
          </label>
          <input
            id="proj-path"
            className="input"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder="/home/me/family.ktree"
          />
        </div>
        <button
          type="button"
          className="btn"
          disabled={!path.trim()}
          onClick={() => run('project.open', t('project.opened'))}
        >
          {t('project.open')}
        </button>
        <button
          type="button"
          className="btn"
          disabled={!path.trim()}
          onClick={() => run('project.create', t('project.created'))}
        >
          {t('project.create')}
        </button>
        <button
          type="button"
          className="btn"
          disabled={!path.trim() || !status.open}
          onClick={() => run('project.backup', t('project.backedUp'))}
        >
          {t('project.backup')}
        </button>
      </div>
    </section>
  )
}

export function ImportExport() {
  const { t } = useTranslation()
  const { status, toast, go } = useApp()
  const fileRef = useRef<HTMLInputElement>(null)
  const [report, setReport] = useState<ImportReport | null>(null)
  const [filter, setFilter] = useState<'all' | 'warning' | 'error' | 'info'>('all')
  const [opt, setOpt] = useState({
    version: '5.5.1',
    charset: 'utf8',
    living: 'include',
    dialect: 'standard',
    include_media: true,
  })

  const onFile = async (f: File | undefined) => {
    if (!f) return
    const bytes = new Uint8Array(await f.arrayBuffer())
    const r = await act<{ report: ImportReport }>('gedcom.import', { data: bytesToBase64(bytes) })
    if (r) {
      setReport(r.report)
      toast(t('import.done', { persons: r.report.persons, issues: r.report.issues.length }))
    }
    if (fileRef.current) fileRef.current.value = ''
  }
  const doExport = async () => {
    try {
      const r = await call<{ data: string; size: number }>('gedcom.export', opt)
      downloadBlob('family-tree.ged', base64ToBytes(r.data) as BlobPart, 'text/plain')
      toast(t('export.done', { size: Math.round(r.size / 1024) }))
    } catch (e) {
      toast(String((e as Error).message), 'error')
    }
  }
  const issues = (report?.issues ?? []).filter((i) => filter === 'all' || i.severity === filter)
  const sel = (k: keyof typeof opt, label: string, options: [string, string][]) => (
    <div>
      <label className="label" htmlFor={`ex-${k}`}>
        {label}
      </label>
      <select
        id={`ex-${k}`}
        className="input"
        value={String(opt[k])}
        onChange={(e) => setOpt({ ...opt, [k]: e.target.value })}
      >
        {options.map(([v, l]) => (
          <option key={v} value={v}>
            {l}
          </option>
        ))}
      </select>
    </div>
  )
  return (
    <div className="mx-auto grid max-w-5xl gap-4 p-6 lg:grid-cols-2">
      <h1 className="sr-only">{t('nav.importexport')}</h1>
      <section className="card p-4">
        <h2 className="mb-2 text-lg font-semibold">{t('import.title')}</h2>
        <p className="mb-3 text-sm text-[var(--muted)]">{t('import.hint')}</p>
        <input
          ref={fileRef}
          type="file"
          accept=".ged,.gedcom,.gedzip,.zip,.gramps,.xml,text/plain"
          aria-label={t('import.choose')}
          className="input"
          onChange={(e) => onFile(e.target.files?.[0])}
          data-testid="import-file"
        />
        {report && (
          <div className="mt-4" data-testid="import-report">
            <div className="mb-2 flex flex-wrap gap-2 text-sm">
              <span className="chip">GEDCOM {report.version}</span>
              <span className="chip">{report.charset}</span>
              <span className="chip">{t('status.persons', { count: report.persons })}</span>
              <span className="chip">{t('status.families', { count: report.families })}</span>
              <span className="chip">{t('import.events', { count: report.events })}</span>
              <span className="chip">{t('import.preserved', { count: report.preserved_structures })}</span>
            </div>
            {status.persons != null && status.persons > report.persons && (
              <p className="mb-2 rounded-lg bg-[var(--accent-soft)] p-2 text-sm" data-testid="merge-note">
                {t('import.mergeNote')}{' '}
                <button type="button" className="underline" onClick={() => go('quality')}>
                  {t('import.findDuplicates')}
                </button>
              </p>
            )}
            <div className="mb-2 flex items-center gap-2">
              <label className="label !mb-0" htmlFor="issue-filter">
                {t('import.show')}
              </label>
              <select
                id="issue-filter"
                className="input !w-auto"
                value={filter}
                onChange={(e) => setFilter(e.target.value as typeof filter)}
              >
                <option value="all">{t('import.all')}</option>
                <option value="error">{t('import.errors')}</option>
                <option value="warning">{t('import.warnings')}</option>
                <option value="info">{t('import.infos')}</option>
              </select>
              <span className="text-sm text-[var(--muted)]">{t('import.issueCount', { count: issues.length })}</span>
            </div>
            <div className="max-h-72 overflow-auto rounded-lg border border-[var(--border)]">
              <table className="w-full text-sm">
                <thead className="sticky top-0 bg-[var(--surface-2)] text-left">
                  <tr>
                    <th className="px-2 py-1">{t('import.line')}</th>
                    <th className="px-2 py-1">{t('import.severity')}</th>
                    <th className="px-2 py-1">{t('import.message')}</th>
                  </tr>
                </thead>
                <tbody>
                  {issues.length === 0 && (
                    <tr>
                      <td colSpan={3} className="px-2 py-2 text-[var(--muted)]">
                        {t('import.clean')}
                      </td>
                    </tr>
                  )}
                  {issues.map((i, n) => (
                    <tr key={n} className="border-t border-[var(--border)]">
                      <td className="px-2 py-1 font-mono text-xs">{i.line || '–'}</td>
                      <td
                        className={`px-2 py-1 ${i.severity === 'error' ? 'text-[var(--danger)]' : i.severity === 'warning' ? 'text-[var(--warn)]' : ''}`}
                      >
                        {t(`import.sev.${i.severity}`)}
                      </td>
                      <td className="px-2 py-1">{i.message}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </section>
      <section className="card p-4">
        <h2 className="mb-2 text-lg font-semibold">{t('export.title')}</h2>
        <div className="grid grid-cols-2 gap-3">
          {sel('version', t('export.version'), [
            ['5.5.1', 'GEDCOM 5.5.1'],
            ['7.0', 'GEDCOM 7.0'],
          ])}
          {sel('charset', t('export.charset'), [
            ['utf8', 'UTF-8'],
            ['utf16', 'UTF-16'],
            ['latin1', 'ISO-8859-1'],
            ['ascii', 'ASCII'],
          ])}
          {sel('living', t('export.living'), [
            ['include', t('export.livingInclude')],
            ['mask', t('export.livingMask')],
            ['exclude', t('export.livingExclude')],
          ])}
          {sel('dialect', t('export.dialect'), [
            ['standard', 'KinTree'],
            ['ancestry', 'Ancestry'],
            ['ftm', 'Family Tree Maker'],
            ['rootsmagic', 'RootsMagic'],
            ['legacy', 'Legacy'],
            ['gramps', 'Gramps'],
          ])}
        </div>
        <label className="mt-3 flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={opt.include_media}
            onChange={(e) => setOpt({ ...opt, include_media: e.target.checked })}
          />
          {t('export.media')}
        </label>
        <button type="button" className="btn btn-primary mt-4" disabled={!status.open} onClick={doExport}>
          {t('export.button')}
        </button>
        <p className="mt-3 text-xs text-[var(--muted)]">{t('export.note')}</p>
      </section>
      <OtherFormats />
      <Packages />
      <ProjectFiles />
    </div>
  )
}
