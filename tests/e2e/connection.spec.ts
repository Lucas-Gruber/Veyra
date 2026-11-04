import { expect, test, type Page } from '@playwright/test'

interface RememberedLogin { serverUrl: string; email: string }
declare global {
  interface Window {
    __VEYRA_LOGIN_PREFERENCES__: (command: string, preferences?: RememberedLogin) => Promise<RememberedLogin | null>
    __VEYRA_SIGNIN_FIXTURE__: { emitPreferencesChanged: () => void; emitQuickBlur: () => void; emitQuickFocus: () => void }
  }
}

// UI contract tests only. Real crypto/network validation uses the Rust tests
// against independently seeded Vaultwarden containers, not this IPC fixture.
async function installIpcFixture(page: Page, initial: RememberedLogin | null = null) {
  // The fake backend lives outside page state, so reloads and native-window
  // stand-ins share the same preferences without using browser storage.
  let releaseLoad: (() => void) | undefined
  const backend = { saved: initial, calls: [] as { command: string; preferences?: RememberedLogin }[], failSave: false, delayNextLoad: false, pendingLoad: false, releaseLoad() { releaseLoad?.(); releaseLoad = undefined } }
  await page.context().exposeBinding('__VEYRA_LOGIN_PREFERENCES__', async (_source, command: string, preferences?: RememberedLogin) => {
    backend.calls.push({ command, ...(preferences ? { preferences: structuredClone(preferences) } : {}) })
    if (command === 'load_login_preferences') {
      const saved = backend.saved ? { ...backend.saved } : null
      if (backend.delayNextLoad) {
        backend.delayNextLoad = false; backend.pendingLoad = true
        await new Promise<void>(resolve => { releaseLoad = resolve })
        backend.pendingLoad = false
      }
      return saved
    }
    if (command === 'save_login_preferences') {
      if (backend.failSave) throw new Error('Synthetic preferences storage unavailable.')
      expect(Object.keys(preferences || {}).sort()).toEqual(['email', 'serverUrl'])
      const url = new URL(preferences!.serverUrl)
      if (url.protocol !== 'https:' || url.username || url.password || url.search || url.hash) throw new Error('Synthetic invalid server address.')
      if (!url.pathname.endsWith('/')) url.pathname += '/'
      backend.saved = { serverUrl: url.href, email: preferences!.email.trim().toLowerCase() }
    } else if (command === 'clear_login_preferences') backend.saved = null
    else throw new Error('Unsupported synthetic preferences command.')
    await Promise.all(page.context().pages().map(target => target.evaluate(() => window.__VEYRA_SIGNIN_FIXTURE__?.emitPreferencesChanged())))
    return backend.saved
  })
  await page.context().addInitScript(() => {
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
    const emitEvent = (event: string) => { for (const id of listeners.get(event) || []) callbacks.get(id)?.({ payload: null }) }
    Object.assign(window, {
      __VEYRA_SIGNIN_FIXTURE__: { emitPreferencesChanged: () => emitEvent('login-preferences:changed'), emitQuickBlur: () => emitEvent('quick-access:blur'), emitQuickFocus: () => emitEvent('quick-access:focus') },
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
          if (['load_login_preferences', 'save_login_preferences', 'clear_login_preferences'].includes(command)) return window.__VEYRA_LOGIN_PREFERENCES__(command, args?.preferences as RememberedLogin | undefined)
          if (command === 'hide_quick_access') return
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
  return backend
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
  const backend = await installIpcFixture(page)
  await page.goto('/')
  await page.getByLabel('Server address', { exact: true }).fill('https://vault.example.test')
  await page.getByLabel('Email address', { exact: true }).fill('fixture@example.test')
  await page.getByLabel('Master password', { exact: true }).fill('ui-fixture-password')
  await page.getByRole('button', { name: 'Connect & unlock' }).click()
  await expect(page.getByLabel('Authenticator code', { exact: true })).toBeVisible()
  await expect(page.getByLabel('Master password', { exact: true })).toHaveValue('')
  expect(backend.saved).toEqual({ serverUrl: 'https://vault.example.test/', email: 'fixture@example.test' })
  await page.getByLabel('Master password', { exact: true }).fill('ui-fixture-password')
  await page.getByLabel('Authenticator code', { exact: true }).fill('123456')
  await page.getByRole('button', { name: 'Verify & unlock' }).click()
  await expect(page.locator('.item-row')).toHaveCount(1)
  await expect(page.locator('.item-row')).toContainText('Connected fixture login')
  await expect(page.locator('.compatibility-notice')).toContainText('2 items use unsupported formats')
  await expect(page.getByRole('button', { name: 'Copy one-time password', exact: true })).toBeDisabled()
  await page.keyboard.press('Control+l')
  await expect(page.getByRole('button', { name: 'Connect & unlock' })).toBeVisible()
  await expect(page.locator('.item-row')).toHaveCount(0)
  await expect(page.getByLabel('Master password', { exact: true })).toHaveValue('')
  await expect(page.getByLabel('Server address', { exact: true })).toHaveValue('https://vault.example.test/')
  await expect(page.getByLabel('Email address', { exact: true })).toHaveValue('fixture@example.test')
  expect(JSON.stringify(backend.calls)).not.toMatch(/ui-fixture-password|123456|twoFactor/)
})

test('a failed sign-in remembers only server and email across reload and can forget them', async ({ page }) => {
  const backend = await installIpcFixture(page)
  await page.goto('/')
  await page.getByLabel('Server address', { exact: true }).fill('https://vault.example.test/subdirectory')
  await page.getByLabel('Email address', { exact: true }).fill('Fixture@Example.test')
  await page.getByLabel('Master password', { exact: true }).fill('wrong-synthetic-password')
  await page.getByRole('button', { name: 'Connect & unlock', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Sign-in failed')
  expect(backend.saved).toEqual({ serverUrl: 'https://vault.example.test/subdirectory/', email: 'fixture@example.test' })
  expect(backend.calls.find(call => call.command === 'save_login_preferences')).toEqual({ command: 'save_login_preferences', preferences: { serverUrl: 'https://vault.example.test/subdirectory', email: 'Fixture@Example.test' } })
  await page.reload()
  await expect(page.getByLabel('Server address', { exact: true })).toHaveValue('https://vault.example.test/subdirectory/')
  await expect(page.getByLabel('Email address', { exact: true })).toHaveValue('fixture@example.test')
  await expect(page.getByLabel('Master password', { exact: true })).toHaveValue('')
  await expect(page.getByLabel('Authenticator code', { exact: true })).toHaveCount(0)
  const storage = await page.evaluate(() => JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } }))
  expect(storage).not.toMatch(/fixture@example|vault\.example|wrong-synthetic-password/)
  await page.getByRole('button', { name: 'Forget saved sign-in details', exact: true }).click()
  await expect(page.getByLabel('Server address', { exact: true })).toHaveValue('')
  await expect(page.getByLabel('Email address', { exact: true })).toHaveValue('')
  expect(backend.saved).toBeNull()
  await page.reload()
  await expect(page.getByLabel('Server address', { exact: true })).toHaveValue('')
  await expect(page.getByRole('button', { name: 'Forget saved sign-in details', exact: true })).toHaveCount(0)
})

test('native Quick Access reloads shared remembered details and discards unsubmitted edits', async ({ page }) => {
  const initial = { serverUrl: 'https://saved.example.test/', email: 'saved@example.test' }
  const backend = await installIpcFixture(page, initial)
  await page.goto('/')
  await expect(page.getByLabel('Email address', { exact: true })).toHaveValue(initial.email)
  const quick = await page.context().newPage()
  await quick.goto('/?window=quick')
  const server = quick.getByLabel('Server address', { exact: true })
  const email = quick.getByLabel('Email address', { exact: true })
  const password = quick.getByLabel('Master password', { exact: true })
  await expect(email).toHaveValue(initial.email)
  await expect(password).toBeFocused()
  await server.fill('https://unsent.example.test')
  await email.fill('unsent@example.test')
  await password.fill('unsent-synthetic-password')
  await quick.evaluate(() => window.__VEYRA_SIGNIN_FIXTURE__.emitQuickBlur())
  await expect(server).toHaveValue('')
  await expect(email).toHaveValue('')
  await expect(password).toHaveValue('')
  expect(backend.saved).toEqual(initial)
  expect(backend.calls.filter(call => call.command === 'save_login_preferences')).toEqual([])
  await page.getByLabel('Server address', { exact: true }).fill('https://changed.example.test')
  await page.getByLabel('Email address', { exact: true }).fill('changed@example.test')
  await page.getByLabel('Master password', { exact: true }).fill('wrong-synthetic-password')
  await page.getByRole('button', { name: 'Connect & unlock', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('Sign-in failed')
  await quick.evaluate(() => window.__VEYRA_SIGNIN_FIXTURE__.emitQuickFocus())
  await expect(server).toHaveValue('https://changed.example.test/')
  await expect(email).toHaveValue('changed@example.test')
  await expect(password).toHaveValue('')
  await expect(password).toBeFocused()
  await quick.getByRole('button', { name: 'Forget saved sign-in details', exact: true }).click()
  await expect(page.getByLabel('Server address', { exact: true })).toHaveValue('')
  await expect(page.getByLabel('Email address', { exact: true })).toHaveValue('')
  expect(backend.saved).toBeNull()
})

test('unavailable preferences storage still permits authentication without retaining secrets', async ({ page }) => {
  const backend = await installIpcFixture(page)
  backend.failSave = true
  await page.goto('/')
  await page.getByLabel('Server address', { exact: true }).fill('https://vault.example.test')
  await page.getByLabel('Email address', { exact: true }).fill('fixture@example.test')
  await page.getByLabel('Master password', { exact: true }).fill('ui-fixture-password')
  await page.getByRole('button', { name: 'Connect & unlock', exact: true }).click()
  await expect(page.getByLabel('Authenticator code', { exact: true })).toBeVisible()
  await expect(page.getByLabel('Master password', { exact: true })).toHaveValue('')
  expect(backend.saved).toBeNull()
  expect(JSON.stringify(backend.calls)).not.toContain('ui-fixture-password')
})

test('a delayed preference load cannot restore a dismissed form or steal focus', async ({ page }) => {
  const initial = { serverUrl: 'https://saved.example.test/', email: 'saved@example.test' }
  const backend = await installIpcFixture(page, initial)
  await page.goto('/?window=quick')
  const password = page.getByLabel('Master password', { exact: true })
  await expect(password).toBeFocused()
  await page.evaluate(() => window.__VEYRA_SIGNIN_FIXTURE__.emitQuickBlur())
  backend.delayNextLoad = true
  await page.evaluate(() => window.__VEYRA_SIGNIN_FIXTURE__.emitQuickFocus())
  await expect.poll(() => backend.pendingLoad).toBe(true)
  await page.evaluate(() => window.__VEYRA_SIGNIN_FIXTURE__.emitQuickBlur())
  const openMain = page.getByRole('button', { name: 'Open Veyra', exact: true })
  await openMain.focus()
  backend.releaseLoad()
  await expect.poll(() => backend.pendingLoad).toBe(false)
  await expect(openMain).toBeFocused()
  await expect(page.getByLabel('Server address', { exact: true })).toHaveValue('')
  await expect(page.getByLabel('Email address', { exact: true })).toHaveValue('')
  await expect(password).toHaveValue('')
  await page.evaluate(() => window.__VEYRA_SIGNIN_FIXTURE__.emitQuickFocus())
  await expect(password).toBeFocused()
  await expect(page.getByLabel('Server address', { exact: true })).toHaveValue(initial.serverUrl)
})
