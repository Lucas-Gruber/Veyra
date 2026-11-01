import { expect, test } from '@playwright/test'
import { installConnectedVault } from './fixtures/connected-vault'

for (const entry of [
  { kind: 'note', title: 'Secure notes', name: 'Synthetic secure note', fields: {} },
  { kind: 'card', title: 'Cards', name: 'Synthetic card', fields: { 'Cardholder name': 'Synthetic cardholder', 'Card number': '4242424242424242', 'Security code': '987', 'Expiry month': '12', 'Expiry year': '2030' } },
  { kind: 'identity', title: 'Identities', name: 'Synthetic identity', fields: { 'First name': 'Synthetic', 'Last name': 'Identity fixture', 'Email': 'identity@example.test', 'Address': 'Synthetic street 42', 'Passport number': 'synthetic-passport' } },
]) {
  test(`${entry.kind} can be created, explicitly viewed and edited without storage persistence`, async ({ page }) => {
    await installConnectedVault(page)
    await page.goto('/')
    await expect(page.locator('.item-row')).toHaveCount(4)
    await page.getByRole('button', { name: 'New item', exact: true }).click()
    let editor = page.getByRole('dialog', { name: 'New item', exact: true })
    await editor.getByRole('combobox', { name: 'Item type', exact: true }).selectOption(entry.kind)
    await editor.getByLabel('Name', { exact: true }).fill(entry.name)
    await editor.getByLabel('Notes', { exact: true }).fill('Synthetic secret contents for explicit reveal')
    for (const [label, value] of Object.entries(entry.fields)) await editor.getByLabel(label, { exact: true }).fill(value)
    await editor.getByRole('button', { name: 'Save item', exact: true }).click()
    await expect(editor).toHaveCount(0)
    await page.getByRole('button', { name: new RegExp(`^${entry.title}`) }).click()
    await page.locator('.item-row').filter({ hasText: entry.name }).click()
    const contents = page.locator('.item-contents')
    await expect(contents).not.toContainText('Synthetic secret contents')
    await contents.getByRole('button', { name: 'Show contents', exact: true }).click()
    await expect(contents).toContainText('Synthetic secret contents for explicit reveal')
    for (const value of Object.values(entry.fields)) await expect(contents).toContainText(value)
    await page.evaluate(() => window.dispatchEvent(new Event('blur')))
    await expect(contents).not.toContainText('Synthetic secret contents')
    await page.getByRole('button', { name: 'Edit item', exact: true }).click()
    editor = page.getByRole('dialog', { name: 'Edit item', exact: true })
    await expect(editor.getByLabel('Notes', { exact: true })).toHaveValue('Synthetic secret contents for explicit reveal')
    for (const [label, value] of Object.entries(entry.fields)) await expect(editor.getByLabel(label, { exact: true })).toHaveValue(value)
    await editor.getByLabel('Notes', { exact: true }).fill('Updated synthetic secret contents')
    await editor.getByRole('button', { name: 'Save item', exact: true }).click()
    await expect(editor).toHaveCount(0)
    await page.keyboard.press('Control+l')
    await expect(page.locator('.item-row')).toHaveCount(0)
    expect(await page.evaluate(() => JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } }))).not.toMatch(/synthetic|fixture|424242|987|passport|example\.test/i)
  })
}
