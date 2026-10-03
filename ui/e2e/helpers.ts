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
