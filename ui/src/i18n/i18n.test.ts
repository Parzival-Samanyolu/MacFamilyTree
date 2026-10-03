import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import en from './en.json'
import tr from './tr.json'

type Tree = { [k: string]: string | Tree }

function flatten(o: Tree, prefix = ''): Record<string, string> {
  const out: Record<string, string> = {}
  for (const [k, v] of Object.entries(o)) {
    if (typeof v === 'string') out[prefix + k] = v
    else Object.assign(out, flatten(v, `${prefix}${k}.`))
  }
  return out
}

function sourceFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f)
    return statSync(p).isDirectory() ? sourceFiles(p) : /\.(tsx?|ts)$/.test(f) && !/\.test\./.test(f) ? [p] : []
  })
}

const E = flatten(en as Tree)
const T = flatten(tr as Tree)

describe('translations', () => {
  it('English and Turkish define exactly the same keys', () => {
    expect(Object.keys(T).sort()).toEqual(Object.keys(E).sort())
  })
  it('has no empty strings', () => {
    for (const [k, v] of [...Object.entries(E), ...Object.entries(T)]) expect(v.trim(), k).not.toBe('')
  })
  it('keeps interpolation placeholders aligned between languages', () => {
    const ph = (s: string) => (s.match(/\{\{\w+\}\}/g) ?? []).sort().join(',')
    for (const k of Object.keys(E)) expect(ph(T[k]), k).toBe(ph(E[k]))
  })
  it('every static t("key") used in the source exists', () => {
    const missing = new Set<string>()
    for (const f of sourceFiles(join(__dirname, '..'))) {
      const text = readFileSync(f, 'utf8')
      for (const m of text.matchAll(/\bt\(\s*['"]([a-zA-Z0-9_.]+)['"]/g)) if (!(m[1] in E)) missing.add(m[1])
    }
    expect([...missing].sort()).toEqual([])
  })
  it('every dynamic key family used in the source has all members', () => {
    const must = [
      'sex.M',
      'sex.F',
      'sex.U',
      'sex.X',
      'childrel.biological',
      'childrel.adopted',
      'childrel.foster',
      'childrel.step',
      'childrel.sealed',
      'famrel.married',
      'import.sev.error',
      'quality.sev.Error',
      'tab.overview',
      'wizard.step2',
    ]
    for (const k of must) expect(E[k], k).toBeTruthy()
    for (const v of [
      'dashboard',
      'persons',
      'tree',
      'fan',
      'relationship',
      'quality',
      'statistics',
      'importexport',
      'settings',
    ])
      expect(E[`nav.${v}`], v).toBeTruthy()
    for (const k of ['BIRT', 'DEAT', 'MARR', 'OCCU', 'EVEN', 'RESI', 'DIV'])
      expect(E[`event.kind.${k}`], k).toBeTruthy()
  })
})
