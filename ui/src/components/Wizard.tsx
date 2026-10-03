import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { act } from '../lib/query'
import { useApp } from '../store/app'
import { Modal } from './Modal'

interface P {
  given: string
  surname: string
  birth: string
}
const empty: P = { given: '', surname: '', birth: '' }

/** Guided first-run flow: you, then your parents. */
export function Wizard() {
  const { t } = useTranslation()
  const { wizardOpen, setWizard, go, select } = useApp()
  const [step, setStep] = useState(0)
  const [me, setMe] = useState<P>(empty)
  const [dad, setDad] = useState<P>(empty)
  const [mom, setMom] = useState<P>(empty)
  const [sex, setSex] = useState('U')
  const people = [me, dad, mom]
  const setters = [setMe, setDad, setMom]
  const cur = people[step]
  const set = setters[step]

  const finish = async () => {
    const created = await act<{ id: string }>('person.create', { given: me.given, surname: me.surname, sex })
    if (!created) return
    if (me.birth)
      await act('event.put', { owner_type: 'person', owner_id: created.id, kind: 'BIRT', date_text: me.birth })
    for (const [kind, p, sx] of [
      ['father', dad, 'M'],
      ['mother', mom, 'F'],
    ] as const) {
      if (!p.given && !p.surname) continue
      const r = await act<{ person_id: string }>('relative.add', {
        person_id: created.id,
        kind,
        given: p.given,
        surname: p.surname,
        sex: sx,
      })
      if (r && p.birth)
        await act('event.put', { owner_type: 'person', owner_id: r.person_id, kind: 'BIRT', date_text: p.birth })
    }
    setWizard(false)
    setStep(0)
    setMe(empty)
    setDad(empty)
    setMom(empty)
    select(created.id)
    go('tree', created.id)
  }

  return (
    <Modal open={wizardOpen} onOpenChange={setWizard} title={t('wizard.title')}>
      <p className="mb-3 text-sm text-[var(--muted)]">{t(`wizard.step${step}`)}</p>
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="label" htmlFor="w-given">
            {t('person.given')}
          </label>
          <input
            id="w-given"
            className="input"
            value={cur.given}
            onChange={(e) => set({ ...cur, given: e.target.value })}
            autoFocus
          />
        </div>
        <div>
          <label className="label" htmlFor="w-surname">
            {t('person.surname')}
          </label>
          <input
            id="w-surname"
            className="input"
            value={cur.surname}
            onChange={(e) => set({ ...cur, surname: e.target.value })}
          />
        </div>
        <div>
          <label className="label" htmlFor="w-birth">
            {t('event.date')}
          </label>
          <input
            id="w-birth"
            className="input"
            value={cur.birth}
            placeholder={t('event.datePlaceholder')}
            onChange={(e) => set({ ...cur, birth: e.target.value })}
          />
        </div>
        {step === 0 && (
          <div>
            <label className="label" htmlFor="w-sex">
              {t('person.sex')}
            </label>
            <select id="w-sex" className="input" value={sex} onChange={(e) => setSex(e.target.value)}>
              <option value="U">{t('sex.U')}</option>
              <option value="M">{t('sex.M')}</option>
              <option value="F">{t('sex.F')}</option>
              <option value="X">{t('sex.X')}</option>
            </select>
          </div>
        )}
      </div>
      <div className="mt-4 flex justify-between">
        <button type="button" className="btn" disabled={step === 0} onClick={() => setStep(step - 1)}>
          {t('common.back')}
        </button>
        {step < 2 ? (
          <button
            type="button"
            className="btn btn-primary"
            disabled={step === 0 && !me.given && !me.surname}
            onClick={() => setStep(step + 1)}
          >
            {t('common.next')}
          </button>
        ) : (
          <button type="button" className="btn btn-primary" onClick={finish}>
            {t('wizard.finish')}
          </button>
        )}
      </div>
    </Modal>
  )
}
