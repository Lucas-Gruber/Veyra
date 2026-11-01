import { expect, test } from '@playwright/test'
import { installConnectedVault } from './fixtures/connected-vault'

test('native settings report system tray availability and can recheck it while locked', async ({ page }) => {
  await installConnectedVault(page)
  await page.goto('/')
  await expect(page.locator('.item-row')).toHaveCount(4)
  await page.getByRole('button', { name: 'Settings', exact: true }).click()
  const settings = page.getByRole('dialog', { name: 'Settings', exact: true })
  await settings.getByRole('button', { name: 'Security', exact: true }).click()
  await expect(settings.getByRole('status')).toHaveText('Synthetic system tray ready.')
  await settings.getByRole('button', { name: 'Check system tray' }).click()
  await page.keyboard.press('Control+l')
  await expect(page.locator('.item-row')).toHaveCount(0)
  await settings.getByRole('button', { name: 'Check system tray' }).click()
  await expect(settings.getByRole('status')).toHaveText('Synthetic system tray ready.')
  expect(await page.evaluate(() => window.__VEYRA_UI_FIXTURE__.calls.filter(call => call.command === 'desktop_integration_status').length)).toBe(3)
})
