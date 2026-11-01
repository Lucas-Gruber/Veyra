import { expect, test } from '@playwright/test'
import { installConnectedVault } from './fixtures/connected-vault'

test('folder tree keeps empty folders, selects descendants and supports keyboard collapse', async ({ page }) => {
  await installConnectedVault(page, {
    folders: [{ id: 'mail', name: 'Work/Accounts/Mail' }, { id: 'empty', name: 'Work/Empty' }, { id: 'similar', name: 'Workshop' }, { id: 'collision', name: 'Favorites' }],
    assignments: { 'personal-login': 'mail', 'platform-login': 'similar', 'river-note': 'collision' },
  })
  await page.goto('/')
  const tree = page.getByRole('tree', { name: 'Folders' })
  const work = tree.getByRole('treeitem').filter({ hasText: /^Work1$/ })
  await work.click()
  await expect(page.locator('.item-row')).toHaveCount(1)
  await expect(page.locator('.item-row')).toContainText('Personal fixture login')
  await work.press('ArrowLeft')
  await expect(tree.getByRole('treeitem')).toHaveCount(3)
  await work.press('ArrowRight')
  await work.press('ArrowRight')
  await expect(tree.getByRole('treeitem').filter({ hasText: /^Accounts1$/ })).toBeFocused()
  await page.getByRole('treeitem').filter({ hasText: /^Empty0$/ }).click()
  await expect(page.locator('.item-row')).toHaveCount(0)
  await tree.getByRole('treeitem').filter({ hasText: /^Favorites1$/ }).click()
  await expect(page.locator('.item-row')).toContainText('River fixture note')
  await page.getByRole('button', { name: /^Favorites/ }).click()
  await expect(page.locator('.item-row')).toHaveCount(0)
  await page.keyboard.press('Control+l')
  await expect(tree).toHaveCount(0)
})
