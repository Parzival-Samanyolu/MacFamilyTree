import { expect, type Page, type APIRequestContext } from '@playwright/test'

export async function resetProject(request: APIRequestContext) {
  await request.post('/api/project.close', { data: {} })
}

export async function openSample(page: Page, size: 'small' | 'large' = 'small') {
  await page.goto('/')
  await page
    .getByRole('button', { name: size === 'small' ? /Explore a small sample/ : /Explore a larger sample/ })
    .click()
  await expect(page.getByTestId('person-list')).toBeVisible()
}

export async function nav(page: Page, name: RegExp | string) {
  await page.getByRole('navigation').getByRole('button', { name }).click()
}

/** Rows of the virtualised people list (not the <option>s of any <select>). */
export function rows(page: Page) {
  return page.getByTestId('person-list').getByRole('option')
}

export async function api<T = Record<string, unknown>>(
  request: APIRequestContext,
  cmd: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  const r = await request.post(`/api/${cmd}`, { data: args })
  const body = await r.json()
  if (!r.ok()) throw new Error(`${cmd}: ${JSON.stringify(body)}`)
  return body as T
}

/** A tiny three-generation family with dates, places and one citation, created through the API. */
export async function seedFamily(request: APIRequestContext) {
  await api(request, 'project.new')
  const me = (await api<{ id: string }>(request, 'person.create', { given: 'Emre', surname: 'Kaya', sex: 'M' })).id
  const dad = await api<{ person_id: string; family_id: string }>(request, 'relative.add', {
    person_id: me,
    kind: 'father',
    given: 'Ali',
  })
  const mom = await api<{ person_id: string }>(request, 'relative.add', {
    person_id: me,
    kind: 'mother',
    given: 'Fatma',
    surname: 'Demir',
  })
  for (const [id, date, place] of [
    [me, '3 Mar 1980', 'Konya, Türkiye'],
    [dad.person_id, '1950', 'Ankara, Türkiye'],
    [mom.person_id, '1955', 'Kars, Türkiye'],
  ])
    await api(request, 'event.put', {
      owner_type: 'person',
      owner_id: id,
      kind: 'BIRT',
      date_text: date,
      place_text: place,
    })
  await api(request, 'event.put', {
    owner_type: 'family',
    owner_id: dad.family_id,
    kind: 'MARR',
    date_text: '12 Jun 1975',
  })
  await api(request, 'event.put', { owner_type: 'person', owner_id: dad.person_id, kind: 'DEAT', date_text: '2010' })
  return { me, dad: dad.person_id, mom: mom.person_id, family: dad.family_id }
}
