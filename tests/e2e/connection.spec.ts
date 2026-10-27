import { expect, test, type Page } from '@playwright/test'

// UI contract tests only. Real crypto/network validation uses the Rust tests
// against independently seeded Vaultwarden containers, not this IPC fixture.
async function installIpcFixture(page: Page) {
  await page.addInitScript(() => {
    let generation = 0
    let locked = true
    let twoFactorRequired = false
    let nextId = 0
    const callbacks = new Map<number, (event: unknown) => void>()
    const listeners = new Map<string, number[]>()
    const status = () => ({ locked, generation, mode: locked ? 'disconnected' : 'connected', lastSync: locked ? null : '2026-10-27T00:00:00Z' })
    const snapshot = () => ({ session: status(), unsupportedItems: locked ? 0 : 2, items: locked ? [] : [{ id: 'fixture-login', name: 'Connected fixture login', username: 'fixture@example.test', url: 'https://example.test/', domain: 'example.test', kind: 'login', favorite: false, folder: 'Fixture folder', initials: 'CF', color: 'violet', updated: '2026-10-27', tags: [], hasPassword: true, hasTotp: false, readOnly: true }] })
    const emit = () => {
      for (const id of listeners.get('vault:changed') || []) callbacks.get(id)?.({ payload: status() })
    }
    Object.assign(window, {
      isTauri: true,
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
      __TAURI_INTERNALS__: {
        transformCallback(callback: (event: unknown) => void) { callbacks.set(++nextId, callback); return nextId },
        unregisterCallback(id: number) { callbacks.delete(id) },
        async invoke(command: string, args?: Record<string, unknown>) {
          if (command === 'plugin:event|listen') {
            const event = String(args?.event)
            listeners.set(event, [...(listeners.get(event) || []), Number(args?.handler)])
            return args?.handler
          }
          if (command === 'plugin:event|unlisten') return
          if (command === 'session_status') return status()
          if (command === 'list_metadata' || command === 'sync_vault') return snapshot()
          if (command === 'lock_vault') { locked = true; generation++; emit(); return status() }
          if (command === 'login_vault') {
            const request = args?.request as { masterPassword: string; twoFactor?: { code: string } }
            if (request.masterPassword !== 'ui-fixture-password') throw { code: 'authentication_failed', message: 'Sign-in failed. Check your credentials.' }
            if (!twoFactorRequired) { twoFactorRequired = true; return { status: 'two_factor_required', providers: ['totp'], session: status() } }
            if (request.twoFactor?.code !== '123456') throw { code: 'authentication_failed', message: 'Sign-in failed. Check your authenticator code.' }
            generation++; locked = false
            return { status: 'authenticated', session: status() }
          }
          throw { code: 'not_supported', message: 'Unsupported UI fixture command.' }
        },
      },
    })
  })
}

test('desktop sign-in failures never open a demo or retain the entered password', async ({ page }) => {
  await installIpcFixture(page)
  await page.goto('/')
  await page.getByLabel('Server address', { exact: true }).fill('https://vault.example.test')
  await page.getByLabel('Email address', { exact: true }).fill('fixture@example.test')
  await page.getByLabel('Master password', { exact: true }).fill('wrong-ui-fixture-password')
  await page.getByRole('button', { name: 'Connect & unlock' }).click()
  await expect(page.getByRole('alert')).toContainText('Sign-in failed')
  await expect(page.getByLabel('Master password', { exact: true })).toHaveValue('')
  await expect(page.locator('.item-row')).toHaveCount(0)
  const storage = await page.evaluate(() => JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } }))
  expect(storage).not.toContain('fixture@example.test')
  expect(storage).not.toContain('wrong-ui-fixture-password')
})

test('desktop 2FA UI re-enters the password, displays partial-vault notice and clears on lock', async ({ page }) => {
  await installIpcFixture(page)
  await page.goto('/')
  await page.getByLabel('Server address', { exact: true }).fill('https://vault.example.test')
  await page.getByLabel('Email address', { exact: true }).fill('fixture@example.test')
  await page.getByLabel('Master password', { exact: true }).fill('ui-fixture-password')
  await page.getByRole('button', { name: 'Connect & unlock' }).click()
  await expect(page.getByLabel('Authenticator code', { exact: true })).toBeVisible()
  await expect(page.getByLabel('Master password', { exact: true })).toHaveValue('')
  await page.getByLabel('Master password', { exact: true }).fill('ui-fixture-password')
  await page.getByLabel('Authenticator code', { exact: true }).fill('123456')
  await page.getByRole('button', { name: 'Verify & unlock' }).click()
  await expect(page.locator('.item-row')).toHaveCount(1)
  await expect(page.locator('.item-row')).toContainText('Connected fixture login')
  await expect(page.locator('.compatibility-notice')).toContainText('2 organization or unsupported items')
  await expect(page.getByRole('button', { name: 'Copy one-time password', exact: true })).toBeDisabled()
  await page.keyboard.press('Control+l')
  await expect(page.getByRole('button', { name: 'Connect & unlock' })).toBeVisible()
  await expect(page.locator('.item-row')).toHaveCount(0)
  await expect(page.getByLabel('Master password', { exact: true })).toHaveValue('')
})
