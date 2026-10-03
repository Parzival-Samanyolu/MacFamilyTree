import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { call } from '../api/client'
import type { Summary } from '../api/types'
import { PersonChip } from '../components/PersonChip'
import { PersonPicker } from '../components/PersonPicker'
import { useApp } from '../store/app'

interface Result {
  descriptions: string[]
  kind: 'blood' | 'spouse' | 'same' | 'affinity' | 'none'
  routes: { up: number; down: number; half: boolean; ancestors: { id: string; name: string }[] }[]
  relatedness: number
  inbreeding_a: number
  inbreeding_b: number
}

const LANGS = ['en', 'tr', 'de', 'es', 'fr', 'ru', 'ar']

export function Relationship() {
  const { t } = useTranslation()
  const { personId, lang: uiLang, select, go } = useApp()
  const { data: current } = useQuery({
    queryKey: ['person.summary', personId],
    queryFn: () => call<Summary>('person.summary', { id: personId }),
    enabled: !!personId,
  })
  const [a, setA] = useState<Summary | null>(null)
  const [b, setB] = useState<Summary | null>(null)
  const [lang, setLang] = useState<string>(uiLang)
  const A = a ?? current ?? null
  const { data, error } = useQuery({
    queryKey: ['relationship.calc', A?.id, b?.id, lang],
    queryFn: () => call<Result>('relationship.calc', { a: A!.id, b: b!.id, lang }),
    enabled: !!A && !!b,
  })
  const open = (id: string) => {
    select(id)
    go('persons', id)
  }
  return (
    <div className="mx-auto max-w-3xl p-6">
      <h1 className="mb-1 text-2xl font-semibold tracking-tight">{t('nav.relationship')}</h1>
      <p className="mb-4 text-sm text-[var(--muted)]">{t('rel.calcHint')}</p>
      <div className="card grid gap-4 p-4 sm:grid-cols-[1fr_auto_1fr]">
        <PersonPicker label={t('rel.personA')} value={A} onChange={setA} />
        <button
          type="button"
          className="btn self-end"
          onClick={() => {
            const x = A
            setA(b)
            setB(x)
          }}
          aria-label={t('rel.swap')}
          disabled={!A || !b}
        >
          ⇄
        </button>
        <PersonPicker label={t('rel.personB')} value={b} onChange={setB} />
      </div>
      <div className="mt-3 flex items-center gap-2">
        <label className="label !mb-0" htmlFor="rel-lang">
          {t('rel.terms')}
        </label>
        <select id="rel-lang" className="input !w-auto" value={lang} onChange={(e) => setLang(e.target.value)}>
          {LANGS.map((l) => (
            <option key={l} value={l}>
              {t(`lang.${l}`)}
            </option>
          ))}
        </select>
      </div>
      {error && <p className="mt-4 text-[var(--danger)]">{String((error as Error).message)}</p>}
      {data && A && b && (
        <section className="card mt-4 p-5" data-testid="rel-result" aria-live="polite">
          {data.kind === 'none' ? (
            <p className="text-lg">{t('rel.unrelated')}</p>
          ) : (
            <>
              <p className="text-lg">
                <button
                  type="button"
                  className="font-medium text-[var(--accent)] hover:underline"
                  onClick={() => open(b.id)}
                >
                  {b.name || t('person.unnamed')}
                </button>{' '}
                {t('rel.is')} <strong data-testid="rel-text">{data.descriptions.join(' / ')}</strong> {t('rel.of')}{' '}
                <button
                  type="button"
                  className="font-medium text-[var(--accent)] hover:underline"
                  onClick={() => open(A.id)}
                >
                  {A.name || t('person.unnamed')}
                </button>
              </p>
              {data.routes.length > 0 && (
                <div className="mt-4">
                  <h2 className="mb-2 text-sm font-semibold uppercase tracking-wide text-[var(--muted)]">
                    {t('rel.commonAncestors')}
                  </h2>
                  <ul className="flex flex-col gap-2 text-sm">
                    {data.routes.map((r, i) => (
                      <li key={i}>
                        <span className="chip mr-2">{t('rel.route', { up: r.up, down: r.down })}</span>
                        {r.half && <span className="chip mr-2">{t('rel.half')}</span>}
                        {r.ancestors.map((x) => (
                          <PersonChip
                            key={x.id}
                            s={{
                              id: x.id,
                              name: x.name,
                              given: x.name,
                              surname: '',
                              sex: 'U',
                              life: '',
                              bookmarked: false,
                              color: null,
                              private: false,
                              birth_text: null,
                              birth_year: null,
                              death_text: null,
                              death_year: null,
                              living: false,
                            }}
                            onClick={() => open(x.id)}
                          />
                        ))}
                      </li>
                    ))}
                  </ul>
                </div>
              )}
              <dl className="mt-4 grid grid-cols-3 gap-3 text-sm">
                <div>
                  <dt className="text-[var(--muted)]">{t('rel.relatedness')}</dt>
                  <dd className="text-xl font-semibold" data-testid="rel-coef">
                    {(data.relatedness * 100).toFixed(data.relatedness < 0.01 ? 2 : 1)}%
                  </dd>
                </div>
                <div>
                  <dt className="text-[var(--muted)]">{t('rel.inbreedingA')}</dt>
                  <dd className="text-xl font-semibold">{(data.inbreeding_a * 100).toFixed(2)}%</dd>
                </div>
                <div>
                  <dt className="text-[var(--muted)]">{t('rel.inbreedingB')}</dt>
                  <dd className="text-xl font-semibold">{(data.inbreeding_b * 100).toFixed(2)}%</dd>
                </div>
              </dl>
            </>
          )}
        </section>
      )}
    </div>
  )
}
