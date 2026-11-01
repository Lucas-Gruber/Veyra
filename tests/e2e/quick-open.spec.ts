import { expect, test } from '@playwright/test'
import { installConnectedVault } from './fixtures/connected-vault'

test('Quick Access has a dedicated open button independent of the result click action', async ({ page }) => {
  await installConnectedVault(page)
  await page.goto('/?window=quick')
  const dialog = page.getByRole('dialog', { name: 'Quick Access' })
  await expect(dialog.getByRole('button', { name: /^Open website:/ })).toHaveCount(3)
  await dialog.getByRole('button', { name: 'Open website: Personal fixture login', exact: true }).click()
  await expect.poll(() => page.evaluate(() => window.__VEYRA_UI_FIXTURE__.calls.some(call => call.command === 'open_website'))).toBe(true)
  const calls = await page.evaluate(() => window.__VEYRA_UI_FIXTURE__.calls.filter(call => ['open_website', 'copy_field', 'begin_browser_fill'].includes(call.command)))
  expect(calls).toEqual([{ command: 'open_website', args: { url: 'https://example.test/' } }])
  await expect.poll(() => page.evaluate(() => window.__VEYRA_UI_FIXTURE__.calls.some(call => call.command === 'hide_quick_access'))).toBe(true)
})
