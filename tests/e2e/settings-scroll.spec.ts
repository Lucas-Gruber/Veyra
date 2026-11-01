import { expect, test } from '@playwright/test'

for (const viewport of [{ width: 1100, height: 540 }, { width: 390, height: 520 }]) {
  test(`settings content scrolls independently at ${viewport.width}px with large text`, async ({ page }) => {
    await page.setViewportSize(viewport)
    await page.addInitScript(() => localStorage.setItem('veyra:preferences', JSON.stringify({ textSize: 18 })))
    await page.goto('/')
    if (viewport.width < 750) await page.getByRole('button', { name: 'Open navigation', exact: true }).click()
    await page.getByRole('button', { name: 'Settings', exact: true }).click()
    const dialog = page.getByRole('dialog', { name: 'Settings', exact: true })
    const body = dialog.getByRole('region', { name: 'Appearance settings' })
    const sidebar = dialog.locator('.settings-sidebar')
    const before = await sidebar.boundingBox()
    await body.evaluate(element => { element.scrollTop = element.scrollHeight })
    await expect.poll(() => body.evaluate(element => element.scrollTop)).toBeGreaterThan(0)
    expect(await sidebar.boundingBox()).toEqual(before)
    await expect(dialog.getByRole('button', { name: 'Close settings', exact: true })).toBeInViewport()
    await expect(dialog.getByRole('button', { name: 'Security', exact: true })).toBeInViewport()
    expect(await dialog.evaluate(element => element.scrollTop)).toBe(0)
    expect(await dialog.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
    await dialog.getByRole('button', { name: 'Security', exact: true }).click()
    await expect(dialog.getByRole('region', { name: 'Security settings' })).toHaveJSProperty('scrollTop', 0)
    await dialog.getByRole('button', { name: 'Close settings', exact: true }).click()
    await expect(dialog).toHaveCount(0)
  })
}
