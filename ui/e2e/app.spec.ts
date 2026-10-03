import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'
import { existsSync, mkdtempSync, readFileSync, readdirSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { nav, openSample, resetProject, rows } from './helpers'

const SAMPLES = join(dirname(fileURLToPath(import.meta.url)), '../../samples/gedcom')

test.beforeEach(async ({ request }) => {
  await resetProject(request)
})

test('welcome screen offers the four ways to start and has no a11y violations', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'Welcome to KinTree' })).toBeVisible()
  for (const name of [/Start a new tree/, /Import a GEDCOM file/, /Explore a small sample/, /Explore a larger sample/])
    await expect(page.getByRole('button', { name })).toBeVisible()
  const results = await new AxeBuilder({ page }).analyze()
  expect(results.violations.map((v) => `${v.id}: ${v.help}`)).toEqual([])
})

test('sample project: browse people, edit a person, add an event with a date preview, undo', async ({ page }) => {
  await openSample(page)
  await expect(rows(page).first()).toBeVisible()
  const first = rows(page).first()
  await first.click()
  await expect(page.getByTestId('person-editor')).toBeVisible()
  const name = await page.getByTestId('person-title').innerText()
  expect(name.length).toBeGreaterThan(0)

  await page.getByRole('tab', { name: /Events & facts/ }).click()
  const before = await page.getByTestId('events-table').locator('tbody tr').count()
  await page.getByRole('button', { name: 'Add event' }).click()
  await page.getByLabel('Type', { exact: true }).selectOption('RESI')
  await page.getByLabel('Date', { exact: true }).fill('abt 3 Mar 1850')
  await expect(page.getByTestId('date-preview')).toContainText('ABT 3 MAR 1850')
  await page.getByLabel('Place', { exact: true }).fill('Konya, Turkey')
  await page.getByRole('button', { name: 'Save' }).click()
  await expect(page.getByRole('dialog')).toBeHidden()
  await expect(page.getByTestId('events-table')).toContainText('Konya, Turkey')
  await expect(page.getByTestId('events-table').locator('tbody tr')).toHaveCount(before + 1)

  // undo from the toolbar removes it again
  await page.getByRole('button', { name: 'Undo' }).click()
  await expect(page.getByTestId('events-table')).not.toContainText('Konya, Turkey')
  // and redo with the keyboard
  await page.getByTestId('person-editor').click({ position: { x: 5, y: 5 } })
  await page.keyboard.press('Control+Shift+Z')
  await expect(page.getByTestId('events-table')).toContainText('Konya, Turkey')
})

test('an unparseable date is flagged but kept as text', async ({ page }) => {
  await openSample(page)
  await rows(page).first().click()
  await page.getByRole('tab', { name: /Events & facts/ }).click()
  await page.getByRole('button', { name: 'Add event' }).click()
  await page.getByLabel('Date', { exact: true }).fill('31 Feb 1850')
  await expect(page.getByTestId('date-preview')).toContainText('not understood')
  await page.getByRole('button', { name: 'Save' }).click()
  await expect(page.getByTestId('events-table')).toContainText('31 Feb 1850')
})

test('guided start creates you and your parents and opens the tree', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: /Start a new tree/ }).click()
  await page.getByLabel('Given name(s)').fill('Emre')
  await page.getByLabel('Surname').fill('Kaya')
  await page.getByLabel('Date').fill('3 Mar 1980')
  await page.getByRole('button', { name: 'Next' }).click()
  await page.getByLabel('Given name(s)').fill('Ali')
  await page.getByRole('button', { name: 'Next' }).click()
  await page.getByLabel('Given name(s)').fill('Fatma')
  await page.getByLabel('Surname').fill('Demir')
  await page.getByRole('button', { name: 'Create my tree' }).click()
  await expect(page.getByTestId('tree-canvas')).toBeVisible()
  await expect(page.getByTestId('tree-card')).toHaveCount(3)
  await expect(page.getByTestId('tree-card').filter({ hasText: 'Ali Kaya' })).toHaveCount(1) // surname inherited from the child
  await expect(page.getByTestId('tree-card').filter({ hasText: 'Fatma Demir' })).toHaveCount(1)
})

test('tree: zoom, keyboard navigation, double-click to edit, set root, export', async ({ page }) => {
  await openSample(page)
  await rows(page).nth(5).click()
  const title = await page.getByTestId('person-title').innerText()
  await nav(page, /Family tree/)
  await expect(page.getByTestId('tree-card').first()).toBeVisible()
  expect(await page.getByTestId('tree-card').count()).toBeGreaterThan(0)

  const zoom = page.getByTestId('zoom-level')
  const z0 = parseInt((await zoom.innerText()).replace('%', ''))
  await page.getByRole('button', { name: 'Zoom in' }).click()
  await expect.poll(async () => parseInt((await zoom.innerText()).replace('%', ''))).toBeGreaterThan(z0)
  await page.getByRole('button', { name: 'Fit' }).click()

  // keyboard navigation moves the selection ring between cards
  await page.getByTestId('tree-canvas').focus()
  const rootCard = page.locator('[data-testid="tree-card"][data-root="1"]')
  await expect(rootCard).toContainText(title.split(' ')[0])
  await page.keyboard.press('ArrowUp')
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('person-editor')).toBeVisible()

  // context menu: set as root re-centres the chart on another person
  await nav(page, /Family tree/)
  const cards = page.getByTestId('tree-card')
  const target = cards.nth(Math.min(1, (await cards.count()) - 1))
  const targetId = await target.getAttribute('data-person')
  await target.click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'Set as root' }).click()
  await expect(page.locator(`[data-testid="tree-card"][data-root="1"]`)).toHaveAttribute('data-person', targetId!)

  const download = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Export', exact: true }).click()
  await page.getByRole('menuitem', { name: 'SVG' }).click()
  const d = await download
  expect(d.suggestedFilename()).toBe('tree.svg')
})

test('tree: changing generations and direction updates the chart', async ({ page }) => {
  await openSample(page)
  await rows(page).nth(3).click()
  await nav(page, /Family tree/)
  await page.getByLabel('Chart type').selectOption('ancestors')
  const n1 = await page.getByTestId('tree-card').count()
  await page.getByRole('button', { name: 'Up +' }).click()
  await page.getByRole('button', { name: 'Up +' }).click()
  await expect.poll(async () => page.getByTestId('tree-card').count()).toBeGreaterThanOrEqual(n1)
  await page.getByLabel('Direction').selectOption('lr')
  await expect(page.getByTestId('tree-card').first()).toBeVisible()
})

test('fan chart renders ancestor slots and responds to generation changes', async ({ page }) => {
  await openSample(page)
  await rows(page).nth(8).click()
  await nav(page, /Fan chart/)
  await expect(page.getByTestId('fan-slot').first()).toBeVisible()
  const slots = await page.getByTestId('fan-slot').count()
  await page.getByLabel('Generations').fill('2')
  await expect(page.getByTestId('fan-gens')).toHaveText('2')
  expect(await page.getByTestId('fan-slot').count()).toBeLessThanOrEqual(slots)
})

test('relationship calculator names the relationship in English and Turkish', async ({ page }) => {
  await openSample(page)
  // pick someone who has a parent: the 10th person is generated deep in the tree
  await rows(page).nth(10).click()
  const childName = await page.getByTestId('person-title').innerText()
  await page.getByRole('tab', { name: 'Relationships' }).click()
  const parentChip = page.getByTestId('person-editor').getByTestId('person-chip').first()
  const hasParent = (await parentChip.count()) > 0
  test.skip(!hasParent, 'sample person without recorded relatives')
  const parentName = (await parentChip.getAttribute('data-name'))!
  await nav(page, /Relationship/)
  await page.getByLabel('Person B').fill(parentName.split(' ')[0])
  await page.getByRole('listbox', { name: 'Person B' }).getByRole('option').first().click()
  await expect(page.getByTestId('rel-result')).toBeVisible()
  const text = await page.getByTestId('rel-text').innerText()
  expect(text).toMatch(/father|mother|parent|grand|uncle|aunt|sibling|brother|sister|cousin|child|son|daughter/)
  await page.getByLabel('Terminology').selectOption('tr')
  await expect(page.getByTestId('rel-text')).not.toHaveText(text)
  void childName
})

test('GEDCOM import shows a detailed, filterable import log for a malformed file', async ({ page }) => {
  await page.goto('/')
  await nav(page, /Import \/ export/)
  await page.getByTestId('import-file').setInputFiles(join(SAMPLES, 'torture/t01_malformed.ged'))
  const report = page.getByTestId('import-report')
  await expect(report).toBeVisible()
  await expect(report).toContainText('malformed line')
  await expect(report).toContainText('dangling pointer')
  await page.getByLabel('Show').selectOption('warning')
  await expect(report.locator('tbody tr').first()).toContainText('Warning')
})

test('GEDCOM import then export round-trips through the browser', async ({ page }) => {
  await page.goto('/')
  await nav(page, /Import \/ export/)
  await page.getByTestId('import-file').setInputFiles(join(SAMPLES, 'clean/02_basic_family.ged'))
  await expect(page.getByTestId('import-report')).toContainText('4 people')
  const download = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Export GEDCOM' }).click()
  const d = await download
  const path = await d.path()
  const text = readFileSync(path!, 'utf8')
  expect(text).toContain('1 NAME John /Smith/')
  expect(text).toContain('2 PLAC Konya, Konya Province, Turkey')
  expect(text).toMatch(/\n4 LATI N37\.8667\n4 LONG E32\.4833/)
})

test('data quality: plausibility findings, ignore, and duplicate detection', async ({ page }) => {
  await page.goto('/')
  await nav(page, /Import \/ export/)
  const ged = [
    '0 HEAD',
    '1 GEDC',
    '2 VERS 5.5.1',
    '0 @I1@ INDI',
    '1 NAME Ahmet /Yılmaz/',
    '1 SEX M',
    '1 BIRT',
    '2 DATE 1900',
    '1 DEAT',
    '2 DATE 1890',
    '0 @I2@ INDI',
    '1 NAME Ahmed /Yilmaz/',
    '1 SEX M',
    '1 BIRT',
    '2 DATE 1900',
    '0 TRLR',
    '',
  ].join('\n')
  await page
    .getByTestId('import-file')
    .setInputFiles({ name: 'q.ged', mimeType: 'text/plain', buffer: Buffer.from(ged) })
  await expect(page.getByTestId('import-report')).toBeVisible()
  await nav(page, /Data quality/)
  const finding = page.getByTestId('finding').filter({ hasText: 'died before being born' })
  await expect(finding).toHaveCount(1)
  await finding.getByRole('button', { name: 'Ignore' }).click()
  await expect(page.getByTestId('finding').filter({ hasText: 'died before being born' })).toHaveCount(0)

  await page.getByRole('tab', { name: 'Duplicates' }).click()
  const pair = page.getByTestId('dup-pair')
  await expect(pair).toHaveCount(1)
  page.once('dialog', (d) => void d.accept())
  await pair
    .getByRole('button', { name: /Keep this one/ })
    .first()
    .click()
  await expect(page.getByTestId('dup-none')).toBeVisible()
  // the merge can be undone from the toast
  await page.getByRole('button', { name: 'Undo' }).first().click()
  await expect(page.getByTestId('dup-pair')).toHaveCount(1)
})

test('command palette jumps to a person and runs commands from the keyboard', async ({ page }) => {
  await openSample(page)
  const firstName = await rows(page).first().locator('.font-medium').innerText()
  await page.keyboard.press('Control+K')
  const input = page.getByRole('combobox')
  await expect(input).toBeFocused()
  await input.fill(firstName.split(' ')[0])
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('person-editor')).toBeVisible()
  await page.keyboard.press('Control+K')
  await page.getByRole('combobox').fill('statistics')
  await page.keyboard.press('Enter')
  await expect(page.getByRole('heading', { name: 'Statistics' })).toBeVisible()
  await page.keyboard.press('Alt+ArrowLeft')
  await expect(page.getByTestId('person-editor')).toBeVisible()
})

test('statistics show counts and drill down to the people behind a bar', async ({ page }) => {
  await openSample(page)
  await nav(page, /Statistics/)
  await expect(page.getByTestId('stat-persons')).not.toHaveText('0')
  await page.getByRole('group', { name: 'Sex' }).getByRole('button').first().click()
  await expect(page.getByRole('dialog')).toBeVisible()
  await expect(page.getByRole('dialog').getByRole('button').nth(1)).toBeVisible()
})

test('Turkish UI: translated labels and Turkish month names in dates', async ({ page }) => {
  await openSample(page)
  await nav(page, /Settings/)
  await page.getByLabel('Language').selectOption('tr')
  await expect(page.getByRole('heading', { name: 'Ayarlar' })).toBeVisible()
  await nav(page, /Kişiler/)
  await rows(page).first().click()
  await page.getByRole('tab', { name: /Olaylar ve olgular/ }).click()
  await page.getByRole('button', { name: 'Olay ekle' }).click()
  await page.getByLabel('Tarih', { exact: true }).fill('3 Mar 1850')
  await expect(page.getByTestId('date-preview')).toContainText('3 Mart 1850')
})

test('appearance: dark theme, high contrast, font scaling; persisted across reload', async ({ page }) => {
  await page.goto('/')
  await nav(page, /Settings/)
  await page.getByLabel('Theme').selectOption('dark')
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await page.getByLabel('High-contrast mode').check()
  await expect(page.locator('html')).toHaveAttribute('data-contrast', 'high')
  await page.getByLabel(/Text size/).fill('1.3')
  await expect.poll(() => page.evaluate(() => getComputedStyle(document.documentElement).fontSize)).toBe('20.8px')
  await page.reload()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await expect(page.locator('html')).toHaveAttribute('data-contrast', 'high')
})

test('main views pass automated accessibility checks (light and dark)', async ({ page }) => {
  await openSample(page)
  await rows(page).nth(4).click()
  for (const [label, probe] of [
    [/People/, 'person-editor'],
    [/Family tree/, 'tree-canvas'],
    [/Statistics/, 'stat-persons'],
    [/Data quality/, null],
    [/Settings/, null],
  ] as const) {
    await nav(page, label)
    if (probe) await expect(page.getByTestId(probe)).toBeVisible()
    const r = await new AxeBuilder({ page }).disableRules(['scrollable-region-focusable']).analyze()
    expect(r.violations.map((v) => `${label}: ${v.id} – ${v.help}`)).toEqual([])
  }
})

test('large sample (2,000 people) lists instantly and searches quickly', async ({ page }) => {
  await openSample(page, 'large')
  const total = await page.locator('text=/\\d[\\d,.]* people/').first().innerText()
  expect(parseInt(total.replace(/\D/g, ''))).toBeGreaterThan(1900)
  const t0 = Date.now()
  await page.getByRole('searchbox').fill('yilmaz')
  await expect(rows(page).first()).toBeVisible()
  expect(Date.now() - t0).toBeLessThan(3000)
  // scrolling the virtual list loads further pages
  await page.getByRole('searchbox').fill('')
  await page.getByTestId('person-list').evaluate((el) => (el.scrollTop = 40000))
  await expect(rows(page).first()).toBeVisible()
})

test('file-backed project: create, autosave, reopen with a rolling backup, manual backup', async ({
  page,
  request,
}) => {
  const dir = mkdtempSync(join(tmpdir(), 'kt-e2e-'))
  const file = join(dir, 'family.ktree')
  await page.goto('/')
  await nav(page, /Import \/ export/)
  await page.getByLabel('File path').fill(file)
  await page.getByRole('button', { name: 'Create new' }).click()
  await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
  expect(existsSync(file)).toBe(true)

  await request.post('/api/person.create', { data: { given: 'Persisted', surname: 'Person', sex: 'F' } })
  await request.post('/api/project.close', { data: {} })
  await page.reload()
  await nav(page, /Import \/ export/)
  await page.getByLabel('File path').fill(file)
  await page.getByRole('button', { name: 'Open', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
  await expect(page.getByText('1 people in 0 families')).toBeVisible()
  expect(readdirSync(join(dir, 'backups')).length).toBe(1)

  await nav(page, /Import \/ export/)
  await page.getByLabel('File path').fill(join(dir, 'copy.bak'))
  await page.getByRole('button', { name: 'Save a backup copy' }).click()
  await expect(page.getByText('Backup written')).toBeVisible()
  expect(existsSync(join(dir, 'copy.bak'))).toBe(true)
})
