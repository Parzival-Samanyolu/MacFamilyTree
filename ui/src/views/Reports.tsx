import { useQuery } from '@tanstack/react-query'
import { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { call } from '../api/client'
import type { PersonDetail } from '../api/types'
import { Modal } from '../components/Modal'
import { downloadBlob } from '../lib/format'
import { act, q } from '../lib/query'
import { useApp } from '../store/app'

interface Generated {
  title: string
  html: string
  markdown: string
  footnotes: number
  index: number
}
interface Tpl {
  lang: string
  key: string
  default: string
  value: string | null
}

type Kind = 'individual' | 'ancestors' | 'descendants' | 'book' | 'family' | 'bibliography'

function Templates({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { t } = useTranslation()
  const [lang, setLang] = useState<'en' | 'tr'>('en')
  const { data } = useQuery({ ...q<Tpl[]>('report.templates'), enabled: open })
  return (
    <Modal open={open} onOpenChange={(o) => !o && onClose()} title={t('reports.templates')} wide>
      <p className="mb-3 text-sm text-[var(--muted)]">{t('reports.templatesHint')}</p>
      <select
        className="input mb-3 !w-auto"
        value={lang}
        onChange={(e) => setLang(e.target.value as 'en' | 'tr')}
        aria-label={t('settings.language')}
      >
        <option value="en">English</option>
        <option value="tr">Türkçe</option>
      </select>
      <div className="flex flex-col gap-3">
        {(data ?? [])
          .filter((x) => x.lang === lang)
          .map((x) => (
            <div key={x.lang + x.key}>
              <label className="label" htmlFor={`tpl-${x.lang}-${x.key}`}>
                {x.key}
              </label>
              <input
                id={`tpl-${x.lang}-${x.key}`}
                className="input font-mono text-xs"
                defaultValue={x.value ?? ''}
                placeholder={x.default}
                onBlur={(e) =>
                  e.target.value !== (x.value ?? '') &&
                  act('report.set_template', { lang: x.lang, key: x.key, value: e.target.value })
                }
              />
            </div>
          ))}
      </div>
    </Modal>
  )
}

export function Reports() {
  const { t } = useTranslation()
  const { personId, lang: uiLang, toast } = useApp()
  const [kind, setKind] = useState<Kind>('individual')
  const [generations, setGenerations] = useState(4)
  const [lang, setLang] = useState<'en' | 'tr'>(uiLang)
  const [privacy, setPrivacy] = useState('off')
  const [family, setFamily] = useState('')
  const [doc, setDoc] = useState<Generated | null>(null)
  const [busy, setBusy] = useState(false)
  const [tplOpen, setTplOpen] = useState(false)
  const frame = useRef<HTMLIFrameElement>(null)
  const { data: person } = useQuery({ ...q<PersonDetail>('person.get', { id: personId }), enabled: !!personId })
  const families = person?.partner_families ?? []
  const needsPerson = kind !== 'bibliography'
  const familyId = family || families[0]?.id || ''

  const generate = async () => {
    setBusy(true)
    try {
      const id = kind === 'family' ? familyId : personId
      const r = await call<Generated>('report.generate', { kind, id, generations, lang, privacy })
      setDoc(r)
    } catch (e) {
      toast((e as Error).message, 'error')
    } finally {
      setBusy(false)
    }
  }
  const slug = (doc?.title ?? 'report')
    .replace(/[^\p{L}\p{N}]+/gu, '-')
    .replace(/^-|-$/g, '')
    .toLowerCase()
  const disabled = busy || (needsPerson && !personId) || (kind === 'family' && !familyId)
  return (
    <div className="flex h-full min-h-0">
      <section
        className="w-80 shrink-0 overflow-auto border-r border-[var(--border)] bg-[var(--surface)] p-4"
        aria-label={t('reports.options')}
      >
        <h1 className="mb-3 text-xl font-semibold tracking-tight">{t('nav.reports')}</h1>
        <div className="flex flex-col gap-3">
          <div>
            <label className="label" htmlFor="rp-kind">
              {t('reports.kind')}
            </label>
            <select id="rp-kind" className="input" value={kind} onChange={(e) => setKind(e.target.value as Kind)}>
              {(['individual', 'ancestors', 'descendants', 'family', 'book', 'bibliography'] as Kind[]).map((k) => (
                <option key={k} value={k}>
                  {t(`reports.type.${k}`)}
                </option>
              ))}
            </select>
          </div>
          {needsPerson && (
            <p className="text-sm">
              {t('reports.forPerson')}:{' '}
              <strong>{person?.summary.name || (personId ? '…' : t('reports.noPerson'))}</strong>
            </p>
          )}
          {kind === 'family' && (
            <div>
              <label className="label" htmlFor="rp-family">
                {t('reports.family')}
              </label>
              <select id="rp-family" className="input" value={familyId} onChange={(e) => setFamily(e.target.value)}>
                {families.length === 0 && <option value="">{t('rel.noPartners')}</option>}
                {families.map((f) => (
                  <option key={f.id} value={f.id}>
                    {[person?.summary.name, ...f.partners.map((p) => p.name)].filter(Boolean).join(' & ')}
                  </option>
                ))}
              </select>
            </div>
          )}
          {(kind === 'ancestors' || kind === 'descendants' || kind === 'book') && (
            <div>
              <label className="label" htmlFor="rp-gen">
                {t('reports.generations', { n: generations })}
              </label>
              <input
                id="rp-gen"
                type="range"
                min={1}
                max={10}
                value={generations}
                onChange={(e) => setGenerations(Number(e.target.value))}
                className="w-full"
              />
            </div>
          )}
          <div>
            <label className="label" htmlFor="rp-lang">
              {t('reports.language')}
            </label>
            <select
              id="rp-lang"
              className="input"
              value={lang}
              onChange={(e) => setLang(e.target.value as 'en' | 'tr')}
            >
              <option value="en">English</option>
              <option value="tr">Türkçe</option>
            </select>
          </div>
          <div>
            <label className="label" htmlFor="rp-privacy">
              {t('reports.privacy')}
            </label>
            <select id="rp-privacy" className="input" value={privacy} onChange={(e) => setPrivacy(e.target.value)}>
              <option value="off">{t('reports.privacyOff')}</option>
              <option value="mask">{t('export.livingMask')}</option>
              <option value="exclude">{t('export.livingExclude')}</option>
            </select>
          </div>
          <button type="button" className="btn btn-primary" onClick={generate} disabled={disabled}>
            {busy ? '…' : t('reports.generate')}
          </button>
          <button type="button" className="btn" onClick={() => setTplOpen(true)}>
            {t('reports.templates')}
          </button>
        </div>
      </section>
      <section className="flex min-w-0 flex-1 flex-col">
        {doc ? (
          <>
            <div className="flex items-center gap-2 border-b border-[var(--border)] bg-[var(--surface)] px-3 py-2">
              <span className="min-w-0 flex-1 truncate font-medium" data-testid="report-title">
                {doc.title}
              </span>
              <span className="chip">{t('reports.footnotes', { count: doc.footnotes })}</span>
              <button type="button" className="btn" onClick={() => downloadBlob(`${slug}.html`, doc.html, 'text/html')}>
                HTML
              </button>
              <button
                type="button"
                className="btn"
                onClick={() => downloadBlob(`${slug}.md`, doc.markdown, 'text/markdown')}
              >
                Markdown
              </button>
              <button type="button" className="btn" onClick={() => frame.current?.contentWindow?.print()}>
                {t('tree.print')}
              </button>
            </div>
            {/* sandboxed: report HTML is generated from user data and must never run scripts */}
            <iframe
              ref={frame}
              title={t('reports.preview')}
              srcDoc={doc.html}
              sandbox="allow-modals allow-same-origin"
              className="min-h-0 flex-1 bg-white"
              data-testid="report-frame"
            />
          </>
        ) : (
          <div className="p-10 text-center text-[var(--muted)]">{t('reports.empty')}</div>
        )}
      </section>
      <Templates open={tplOpen} onClose={() => setTplOpen(false)} />
    </div>
  )
}
