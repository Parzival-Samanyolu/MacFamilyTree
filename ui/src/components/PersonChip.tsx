import type { Summary } from '../api/types'
import { initials, sexColor } from '../lib/format'
import { useTranslation } from 'react-i18next'

export function Avatar({ s, size = 32 }: { s: Pick<Summary, 'given' | 'surname' | 'sex' | 'color'>; size?: number }) {
  return (
    <span
      aria-hidden="true"
      className="inline-flex shrink-0 items-center justify-center rounded-full text-white"
      style={{ width: size, height: size, background: s.color || sexColor(s.sex), fontSize: size * 0.38 }}
    >
      {initials(s)}
    </span>
  )
}

export function PersonChip({ s, onClick }: { s: Summary; onClick?: () => void }) {
  const { t } = useTranslation()
  const inner = (
    <>
      <Avatar s={s} size={26} />
      <span className="truncate">{s.name || t('person.unnamed')}</span>
      {s.life && <span className="text-xs text-[var(--muted)]">{s.life}</span>}
    </>
  )
  return onClick ? (
    <button
      type="button"
      className="inline-flex max-w-full items-center gap-2 rounded-full border border-[var(--border)] bg-[var(--surface)] py-0.5 pl-0.5 pr-3 text-sm hover:bg-[var(--surface-2)]"
      onClick={onClick}
      data-testid="person-chip"
      data-name={s.name}
    >
      {inner}
    </button>
  ) : (
    <span className="inline-flex max-w-full items-center gap-2 text-sm" data-testid="person-chip" data-name={s.name}>
      {inner}
    </span>
  )
}
