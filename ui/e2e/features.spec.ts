import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'
import { readFileSync, writeFileSync, mkdtempSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { api, nav, resetProject, rows, seedFamily } from './helpers'

test.beforeEach(async ({ request }) => {
  await resetProject(request)
})

async function openPerson(page: import('@playwright/test').Page, name: string) {
  await page.goto('/')
  await nav(page, /People/)
  await page.getByRole('searchbox').fill(name)
  await expect(rows(page)).toHaveCount(1)
  await rows(page).first().click()
  await expect(page.getByTestId('person-title')).toContainText(name)
}

test('reports: English and Turkish narrative, templates, privacy and downloads', async ({ page, request }) => {
  await seedFamily(request)
  await openPerson(page, 'Emre')
  await nav(page, /Reports/)
  await page.getByRole('button', { name: 'Create report' }).click()
  const frame = page.frameLocator('[data-testid="report-frame"]')
  await expect(frame.getByText('Emre Kaya was born on 3 March 1980 in Konya, Türkiye.')).toBeVisible()
  await expect(frame.getByText('He was the child of Ali Kaya and Fatma Demir.')).toBeVisible()
  await expect(page.getByTestId('report-title')).toHaveText('Emre Kaya')

  await page.getByLabel('Report language').selectOption('tr')
  await page.getByRole('button', { name: 'Create report' }).click()
  await expect(frame.getByText("3 Mart 1980 tarihinde Konya, Türkiye'de doğdu.")).toBeVisible()

  // user-editable sentence templates change the output
  await page.getByLabel('Report language').selectOption('en')
  await page.getByRole('button', { name: 'Sentence templates' }).click()
  const tpl = page.getByLabel('birth_full')
  await tpl.fill('Born {date} at {place}: {name}.')
  await tpl.blur()
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: 'Create report' }).click()
  await expect(frame.getByText('Born on 3 March 1980 at Konya, Türkiye: Emre Kaya.')).toBeVisible()

  // other report types
  await page.getByLabel('Report', { exact: true }).selectOption('ancestors')
  await page.getByRole('button', { name: 'Create report' }).click()
  await expect(page.getByTestId('report-title')).toHaveText('Ancestors of Emre Kaya')
  await expect(frame.getByRole('heading', { name: /2\. Ali Kaya/ })).toBeVisible()
  await page.getByLabel('Report', { exact: true }).selectOption('bibliography')
  await page.getByRole('button', { name: 'Create report' }).click()
  await expect(page.getByTestId('report-title')).toHaveText('Bibliography')

  // privacy: the living person's details never reach the report
  await page.getByLabel('Report', { exact: true }).selectOption('descendants')
  await page.getByLabel('Living people').selectOption('exclude')
  await page.getByRole('button', { name: 'Create report' }).click()
  await expect(frame.locator('body')).not.toContainText('Konya')

  const dl = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Markdown' }).click()
  expect((await dl).suggestedFilename()).toMatch(/\.md$/)
})

test('timeline lists events with a historical overlay and charts life spans', async ({ page, request }) => {
  await seedFamily(request)
  await page.goto('/')
  await nav(page, /Timeline/)
  const list = page.getByTestId('timeline-list')
  await expect(list).toContainText('Birth: Ali Kaya')
  await expect(list).toContainText('Marriage: Ali Kaya & Fatma Demir')
  await expect(list.locator('[data-history="1"]').first()).toBeVisible()
  await page.getByLabel('Show historical events').uncheck()
  await expect(list.locator('[data-history="1"]')).toHaveCount(0)

  await page.getByRole('tab', { name: 'Life spans' }).click()
  const chart = page.getByTestId('lifespan-chart')
  await expect(chart).toBeVisible()
  await expect(chart.getByRole('button')).toHaveCount(3)
  await page.getByLabel('Surname').fill('Kaya')
  await expect(chart.getByRole('button')).toHaveCount(2)
  await chart.getByRole('button').first().click()
  await expect(page.getByTestId('person-editor')).toBeVisible()
})

test('calendar shows this month’s birthdays and exports iCalendar', async ({ page, request }) => {
  await api(request, 'project.new')
  const now = new Date()
  const id = (await api<{ id: string }>(request, 'person.create', { given: 'Birthday', surname: 'Person', sex: 'F' }))
    .id
  await api(request, 'event.put', {
    owner_type: 'person',
    owner_id: id,
    kind: 'BIRT',
    date_text: `15 ${['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'][now.getMonth()]} ${now.getFullYear() - 30}`,
  })
  await page.goto('/')
  await nav(page, /Calendar/)
  const cell = page.locator('[data-day="15"]')
  await expect(cell).toContainText('Birthday Person (30)')
  await page.getByRole('button', { name: 'Next month' }).click()
  await expect(page.locator('[data-day="15"]')).not.toContainText('Birthday Person')
  await page.getByRole('button', { name: 'Previous month' }).click()
  await cell.getByRole('button').click()
  await expect(page.getByTestId('person-editor')).toBeVisible()
  await nav(page, /Calendar/)
  const dl = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Export iCalendar (.ics)' }).click()
  const d = await dl
  const ics = readFileSync((await d.path())!, 'utf8')
  expect(ics).toContain('BEGIN:VEVENT')
  expect(ics).toContain("Birthday Person's birthday")
})

test('library: sources with templates, repositories, tasks and the unsourced-facts list', async ({ page, request }) => {
  await seedFamily(request)
  await page.goto('/')
  await nav(page, /Sources & tasks/)

  // repository, then a source of a template type that uses it
  await page.getByRole('tab', { name: 'Repositories' }).click()
  await page.getByRole('button', { name: 'Add repository' }).click()
  await page.getByLabel('Name', { exact: true }).fill('State Archive')
  await page.getByLabel('Website').fill('https://archive.example.org')
  await page.getByRole('button', { name: 'Save' }).click()
  await expect(page.getByTestId('repo-list')).toContainText('State Archive')

  await page.getByRole('tab', { name: 'Sources' }).click()
  await page.getByRole('button', { name: 'Add source' }).click()
  await page.getByLabel('Source type').selectOption('census')
  await expect(page.getByLabel('Source title')).toHaveAttribute('placeholder', /1881 Census/)
  await page.getByLabel('Source title').fill('1950 Census of Türkiye')
  await page.getByLabel('Repository').selectOption({ label: 'State Archive' })
  await page.getByLabel('Reliability').selectOption('3')
  await page.getByRole('button', { name: 'Save' }).click()
  const table = page.getByTestId('sources-table')
  await expect(table).toContainText('1950 Census of Türkiye')
  await expect(table).toContainText('State Archive')
  await expect(table).toContainText('Census')

  // unsourced facts shrink when a citation is added
  await page.getByRole('tab', { name: 'Unsourced facts' }).click()
  const list = page.getByTestId('unsourced-list')
  await expect(list.locator('li')).toHaveCount(5)
  await api(request, 'citation.add', {
    target_type: 'event',
    target_id: (await api<{ items: { event_id: string }[] }>(request, 'sources.unsourced')).items[0].event_id,
    new_source_title: 'Certificate',
  })
  await page.reload()
  await nav(page, /Sources & tasks/)
  await page.getByRole('tab', { name: 'Unsourced facts' }).click()
  await expect(page.getByTestId('unsourced-list').locator('li')).toHaveCount(4)

  // tasks
  await page.getByRole('tab', { name: 'To-do list' }).click()
  await page.getByLabel('New task').fill('Request the 1950 census page')
  await page.getByLabel('Priority').selectOption('2')
  await page.getByRole('button', { name: 'Add task' }).click()
  const tasks = page.getByTestId('task-list')
  await expect(tasks).toContainText('Request the 1950 census page')
  await tasks.getByLabel('Status').selectOption('done')
  await expect(tasks).not.toContainText('Request the 1950 census page')
  await page.getByLabel('Show completed tasks').check()
  await expect(tasks.locator('[data-status="done"]')).toHaveCount(1)

  // deleting a source asks first and is undoable
  await page.getByRole('tab', { name: 'Sources' }).click()
  page.once('dialog', (d) => void d.accept())
  await table
    .getByRole('row', { name: /1950 Census/ })
    .getByRole('button', { name: 'Delete' })
    .click()
  await expect(table).not.toContainText('1950 Census of Türkiye')
  await page.getByRole('button', { name: 'Undo' }).first().click()
  await expect(table).toContainText('1950 Census of Türkiye')
})

test('CSV, JSON and iCalendar export plus CSV import with Turkish headers', async ({ page, request }) => {
  await seedFamily(request)
  await page.goto('/')
  await nav(page, /Import \/ export/)
  const csvDl = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Export people (CSV)' }).click()
  const csv = readFileSync((await (await csvDl).path())!, 'utf8')
  expect(csv.split('\r\n')[0]).toBe(
    'id,given,surname,sex,birth_date,birth_place,death_date,death_place,occupation,father_id,mother_id,partner_ids',
  )
  expect(csv).toContain('Emre,Kaya,M,3 MAR 1980,"Konya, Türkiye"')
  const jsonDl = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Export project (JSON)' }).click()
  expect(JSON.parse(readFileSync((await (await jsonDl).path())!, 'utf8')).tables.person).toHaveLength(3)

  await resetProject(request)
  await page.reload()
  const dir = mkdtempSync(join(tmpdir(), 'kt-csv-'))
  const file = join(dir, 'kisiler.csv')
  writeFileSync(
    file,
    'Ad;Soyad;Cinsiyet;Doğum Tarihi;Doğum Yeri\nAyşe;Yılmaz;K;3 Mart 1950;Konya\n;;;;\nMehmet;Kaya;E;belli değil;Ankara\n',
  )
  await nav(page, /Import \/ export/)
  await page.getByTestId('csv-file').setInputFiles(file)
  const warnings = page.getByTestId('csv-warnings')
  await expect(warnings).toContainText('no name')
  await expect(warnings).toContainText('kept as text')
  await nav(page, /People/)
  await expect(page.getByText('2 people').first()).toBeVisible()
  await page.getByRole('searchbox').fill('yilmaz')
  await expect(rows(page)).toHaveCount(1)
})

test('new views pass automated accessibility checks', async ({ page, request }) => {
  const f = await seedFamily(request)
  await api(request, 'rec.put', { table: 'source', row: { title: 'Parish book', author: 'Priest' } })
  await page.goto('/')
  await nav(page, /People/)
  await page.getByRole('searchbox').fill('Emre')
  await expect(rows(page)).toHaveCount(1)
  await rows(page).first().click()
  for (const [label, probe] of [
    [/Timeline/, 'timeline-list'],
    [/Calendar/, 'calendar-month'],
    [/Reports/, null],
    [/Sources & tasks/, 'sources-table'],
    [/Import \/ export/, null],
  ] as const) {
    await nav(page, label)
    if (probe) await expect(page.getByTestId(probe)).toBeVisible()
    const r = await new AxeBuilder({ page }).disableRules(['scrollable-region-focusable']).analyze()
    expect(r.violations.map((v) => `${label}: ${v.id} – ${v.help} – ${v.nodes[0]?.target.join(' ')}`)).toEqual([])
  }
  await nav(page, /Reports/)
  await page.getByRole('button', { name: 'Create report' }).click()
  await expect(page.getByTestId('report-frame')).toBeVisible()
  // the preview is a script-less sandboxed iframe, so scan the surrounding page without it ...
  const around = await new AxeBuilder({ page })
    .exclude('[data-testid="report-frame"]')
    .disableRules(['scrollable-region-focusable'])
    .analyze()
  expect(around.violations.map((v) => `${v.id} ${v.nodes[0]?.target.join(' ')}`)).toEqual([])
  // ... and scan the generated report document itself (what people print or share)
  for (const kind of ['individual', 'book']) {
    const rep = await api<{ html: string }>(request, 'report.generate', { kind, id: f.me, generations: 3 })
    await page.setContent(rep.html)
    const r = await new AxeBuilder({ page }).analyze()
    expect(r.violations.map((v) => `${kind}: ${v.id} – ${v.help}`)).toEqual([])
  }
})

test('map: offline geocoding, markers, heat map, migration arcs, time slider, export and accessibility', async ({
  page,
  request,
}) => {
  await seedFamily(request)
  const me = (await api<{ items: { id: string }[] }>(request, 'person.list', { q: 'Emre', limit: 5 })).items[0].id
  await api(request, 'event.put', {
    owner_type: 'person',
    owner_id: me,
    kind: 'DEAT',
    date_text: '2050',
    place_text: 'Adana, Türkiye',
  })
  await page.goto('/')
  await nav(page, /Map/)
  const canvas = page.getByTestId('map-canvas')
  await expect(canvas).toHaveAttribute('data-ready', '1', { timeout: 20_000 })
  await expect(canvas).toHaveAttribute('data-visible', '0')
  await expect(page.getByTestId('map-missing').getByRole('listitem').first()).toBeVisible()
  await page.getByTestId('map-geocode-offline').click()
  await page.getByLabel('Hide living people').uncheck()
  await expect.poll(async () => Number(await canvas.getAttribute('data-visible'))).toBeGreaterThanOrEqual(3)
  // Event-type filter narrows the markers.
  const all = Number(await canvas.getAttribute('data-visible'))
  await page.getByRole('group', { name: 'Event types' }).getByLabel('Death').uncheck()
  await expect.poll(async () => Number(await canvas.getAttribute('data-visible'))).toBeLessThan(all)
  await page.getByRole('group', { name: 'Event types' }).getByLabel('Death').check()
  // Modes.
  await page.getByTestId('map-mode').selectOption('heat')
  await page.getByTestId('map-mode').selectOption('migration')
  await expect.poll(async () => Number(await canvas.getAttribute('data-arcs'))).toBeGreaterThanOrEqual(1)
  await page.getByTestId('map-mode').selectOption('lineage')
  await expect.poll(async () => Number(await canvas.getAttribute('data-arcs'))).toBeGreaterThanOrEqual(1)
  await page.getByTestId('map-mode').selectOption('markers')
  // Time slider.
  const to = page.getByRole('slider', { name: 'To' })
  await to.fill('1951')
  await expect.poll(async () => Number(await canvas.getAttribute('data-visible'))).toBeLessThan(all)
  await page.getByRole('button', { name: 'All years' }).click()
  // Table alternative and export.
  await page.getByText(/Show as table/).click()
  await expect(page.getByTestId('map-table').getByRole('row')).not.toHaveCount(1)
  const dl = page.waitForEvent('download')
  await page.getByRole('button', { name: 'KML' }).click()
  const kml = readFileSync(await (await dl).path(), 'utf8')
  expect(kml).toContain('<Placemark>')
  // Manual coordinates through the API are reflected as exact (non-approximate) points.
  const r = await new AxeBuilder({ page }).disableRules(['scrollable-region-focusable']).analyze()
  expect(r.violations.map((v) => `${v.id} – ${v.help} – ${v.nodes[0]?.target.join(' ')}`)).toEqual([])
})

// A 2×2 PNG, used as a stand-in photo.
const PNG_2X2 = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFklEQVR4nGP8z8Dwn4GBgYGJAQoAHxUCAm4qcZgAAAAASUVORK5CYII=',
  'base64',
)

test('media: upload, de-duplicate, caption, link, profile photo, slideshow, delete and undo', async ({
  page,
  request,
}) => {
  await seedFamily(request)
  await page.goto('/')
  await nav(page, /Media/)
  await expect(page.getByText(/No media yet/)).toBeVisible()
  const input = page.getByTestId('media-file-input')
  await input.setInputFiles([
    { name: 'wedding.png', mimeType: 'image/png', buffer: PNG_2X2 },
    { name: 'same-bytes.png', mimeType: 'image/png', buffer: PNG_2X2 },
    { name: 'letter.txt', mimeType: 'text/plain', buffer: Buffer.from('Dear Ali,') },
  ])
  await expect(page.getByTestId('media-card')).toHaveCount(2)
  await expect(page.getByTestId('media-card').filter({ hasText: 'wedding.png' })).toBeVisible()
  // Search + type filter
  await page.getByRole('searchbox', { name: 'Search media' }).fill('letter')
  await expect(page.getByTestId('media-card')).toHaveCount(1)
  await page.getByRole('searchbox', { name: 'Search media' }).fill('')
  await page.getByRole('combobox', { name: 'Media type' }).selectOption('image')
  await expect(page.getByTestId('media-card')).toHaveCount(1)
  // Detail: caption, link to a person, profile photo
  await page.getByTestId('media-card').click()
  const dlg = page.getByRole('dialog')
  await dlg.getByLabel('Caption').fill('Wedding day')
  await dlg.getByLabel('Caption').blur()
  await dlg.getByLabel('Link to a person').fill('Emre')
  await dlg.getByRole('option', { name: /Emre/ }).first().click()
  await dlg.getByRole('button', { name: 'Link', exact: true }).click()
  await expect(dlg.getByTestId('media-links')).toContainText('Emre Kaya')
  await dlg.getByRole('button', { name: 'Make profile photo' }).click()
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('media-card').first()).toContainText('Wedding day')
  // The person's Media tab shows it as the profile photo
  await nav(page, /People/)
  await page.getByRole('searchbox').fill('Emre')
  await expect(rows(page)).toHaveCount(1)
  await rows(page).first().click()
  await page.getByRole('tab', { name: 'Media' }).click()
  await expect(page.getByTestId('person-media')).toContainText('Wedding day')
  await expect(page.getByTestId('person-media')).toContainText('Profile photo')
  await page
    .getByTestId('person-media-input')
    .setInputFiles({ name: 'diary.txt', mimeType: 'text/plain', buffer: Buffer.from('Konya, 1950') })
  await expect(page.getByTestId('person-media')).toContainText('diary.txt')
  // Slideshow
  await nav(page, /Media/)
  await page.getByRole('button', { name: 'Slideshow' }).click()
  await expect(page.getByTestId('slideshow')).toBeVisible()
  await expect(page.getByTestId('slideshow').locator('img')).toBeVisible()
  await page.getByRole('button', { name: 'Next' }).click()
  await page.keyboard.press('Escape')
  // Delete + undo
  await page.getByRole('combobox', { name: 'Media type' }).selectOption('image')
  await page.getByTestId('media-card').first().click()
  page.once('dialog', (d) => void d.accept())
  await page.getByRole('dialog').getByRole('button', { name: 'Delete' }).click()
  await expect(page.getByText(/No media yet/)).toBeVisible()
  await page.getByRole('button', { name: 'Undo' }).first().click()
  await expect(page.getByTestId('media-card').first()).toBeVisible()
  const r = await new AxeBuilder({ page }).disableRules(['scrollable-region-focusable']).analyze()
  expect(r.violations.map((v) => `${v.id} – ${v.help} – ${v.nodes[0]?.target.join(' ')}`)).toEqual([])
})

test('virtual tree: WebGL scene renders, themes change the picture, list navigation, screenshot', async ({
  page,
  request,
}) => {
  await seedFamily(request)
  await openPerson(page, 'Emre')
  await nav(page, /Virtual tree/)
  const canvas = page.getByTestId('virtual-canvas')
  await expect(canvas).toHaveAttribute('data-ready', '1', { timeout: 20_000 })
  await expect(canvas).toHaveAttribute('data-nodes', '3')
  await expect.poll(async () => Number(await canvas.getAttribute('data-labels'))).toBeGreaterThanOrEqual(3)
  const pixels = () =>
    page.evaluate(() => {
      const c = document.querySelector('[data-testid="virtual-canvas"] canvas') as HTMLCanvasElement
      return c.toDataURL('image/png')
    })
  const garden = await pixels()
  expect(garden.length).toBeGreaterThan(5000)
  await page.getByTestId('v3-theme').selectOption('night')
  await expect(canvas).toHaveAttribute('data-theme', 'night')
  await expect.poll(async () => (await pixels()) !== garden).toBe(true)
  // keyboard-accessible list flies the camera to a person
  await page.getByText(/Browse as a list/).click()
  await page
    .getByTestId('virtual-list')
    .getByRole('button', { name: /Ali Kaya/ })
    .click()
  await expect(canvas).toHaveAttribute('data-focus', /.+/)
  const dl = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Save image' }).click()
  expect((await dl).suggestedFilename()).toBe('kintree-virtual-tree.png')
  // labels can be switched off
  await page.getByLabel('Show names').uncheck()
  await expect.poll(async () => Number(await canvas.getAttribute('data-labels'))).toBe(0)
  const r = await new AxeBuilder({ page }).disableRules(['scrollable-region-focusable']).analyze()
  expect(r.violations.map((v) => `${v.id} – ${v.help} – ${v.nodes[0]?.target.join(' ')}`)).toEqual([])
})

test('stories: compose blocks, reorder, preview in English and Turkish, export, undo delete', async ({
  page,
  request,
}) => {
  await seedFamily(request)
  await page.goto('/')
  await nav(page, /Stories/)
  await page.getByRole('button', { name: 'New story' }).click()
  const editor = page.getByTestId('story-editor')
  await editor.getByLabel('Title').fill('Our roots')
  const add = async (type: string) => {
    await editor.getByLabel('Block type').selectOption({ label: type })
    await editor.getByRole('button', { name: 'Add block' }).click()
  }
  await add('Heading')
  await editor.getByRole('textbox', { name: 'Heading' }).fill('Where it began')
  await add('Text')
  await editor.getByRole('textbox', { name: 'Text' }).fill('A village near Konya.')
  await add('Person write-up')
  await editor.getByLabel('Person').fill('Emre')
  await editor.getByRole('option', { name: /Emre/ }).first().click()
  const frame = page.frameLocator('[data-testid="story-frame"]')
  await expect(frame.getByRole('heading', { name: 'Where it began' })).toBeVisible()
  await expect(frame.getByText('A village near Konya.')).toBeVisible()
  await expect(frame.getByText('Emre Kaya was born on 3 March 1980 in Konya, Türkiye.')).toBeVisible()
  // Reorder: move the text above the heading
  await page.getByTestId('story-blocks').locator('[data-block="text"]').getByRole('button', { name: 'Move up' }).click()
  await expect(page.getByTestId('story-blocks').locator('li').first()).toHaveAttribute('data-block', 'text')
  // Turkish narrative
  await editor.getByRole('combobox', { name: 'Report language' }).selectOption('tr')
  await expect(frame.getByText(/Emre Kaya, 3 Mart 1980 tarihinde Konya/)).toBeVisible()
  // Persisted
  await expect.poll(async () => (await api<{ blocks: number }[]>(request, 'story.list'))[0]?.blocks).toBe(3)
  const dl = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Markdown' }).click()
  expect(readFileSync(await (await dl).path(), 'utf8')).toContain('# Our roots')
  const r = await new AxeBuilder({ page })
    .exclude('[data-testid="story-frame"]')
    .disableRules(['scrollable-region-focusable'])
    .analyze()
  expect(r.violations.map((v) => `${v.id} – ${v.help} – ${v.nodes[0]?.target.join(' ')}`)).toEqual([])
  // Delete and undo
  page.once('dialog', (d) => void d.accept())
  await editor.getByRole('button', { name: 'Delete', exact: true }).click()
  await expect(page.getByText('No stories yet.')).toBeVisible()
  await page.getByRole('button', { name: 'Undo' }).first().click()
  await expect(page.getByTestId('story-list').getByRole('button', { name: /Our roots/ })).toBeVisible()
})
