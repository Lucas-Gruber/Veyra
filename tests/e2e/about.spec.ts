import { readFileSync } from 'node:fs'
import { expect, test, type Page } from '@playwright/test'

const currentVersion = (JSON.parse(readFileSync(new URL('../../package.json', import.meta.url), 'utf8')) as { version: string }).version
async function mockProgramInfo(page: Page, scenario: 'available' | 'no_releases' | 'failure', automatic = true) {
  await page.addInitScript(({ version, state, enabled }) => {
    const calls: string[] = []
    let callbackId = 0
    const session = { locked: true, mode: 'disconnected', generation: 0, lastSync: null }
    localStorage.setItem('veyra:preferences', JSON.stringify({ automaticUpdateChecks: enabled }))
    Object.assign(window, {
      isTauri: true,
      __VEYRA_ABOUT_TEST__: { calls },
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
      __TAURI_INTERNALS__: {
        transformCallback() { return ++callbackId },
        unregisterCallback() {},
        async invoke(command: string) {
          calls.push(command)
          if (command === 'plugin:event|listen') return callbackId
          if (command === 'plugin:event|unlisten') return
          if (command === 'session_status') return session
          if (command === 'app_info') return { version, channel: 'prerelease', platform: 'linux', architecture: 'x86_64', license: 'GPL-3.0-only', repositoryUrl: 'https://github.com/Lucas-Gruber/Veyra' }
          if (command === 'check_for_updates') {
            if (state === 'failure') throw { code: 'network_error', message: 'Could not reach the release service securely.' }
            return { state, currentVersion: version, latestVersion: state === 'available' ? '9.0.0' : null, releaseUrl: state === 'available' ? 'https://github.com/Lucas-Gruber/Veyra/releases/tag/v9.0.0' : null, publishedAt: state === 'available' ? '2026-10-27T10:00:00Z' : null, checkedAt: new Date().toISOString() }
          }
          if (command === 'open_release_page') return
          throw { code: 'unexpected_command', message: 'Unsupported About UI fixture command.' }
        },
      },
    })
  }, { version: currentVersion, state: scenario, enabled: automatic })
}
async function showAbout(page: Page) {
  await page.getByRole('button', { name: 'Settings', exact: true }).click()
  await page.getByRole('button', { name: 'About', exact: true }).click()
  return page.getByRole('dialog', { name: 'Settings' })
}
async function checkCalls(page: Page) {
  return page.evaluate(() => {
    const fixture = (window as unknown as { __VEYRA_ABOUT_TEST__: { calls: string[] } }).__VEYRA_ABOUT_TEST__
    return fixture.calls.filter(command => command === 'check_for_updates').length
  })
}

test('About shows the build version and makes no update request in browser preview', async ({ page }) => {
  const remoteRequests: string[] = []
  page.on('request', request => { if (new URL(request.url()).hostname === 'api.github.com') remoteRequests.push(request.url()) })
  await page.goto('/')
  const about = await showAbout(page)
  await expect(about.getByText(`Version ${currentVersion}`, { exact: true })).toBeVisible()
  await expect(about.getByText('Browser preview', { exact: true })).toBeVisible()
  await expect(about.getByRole('button', { name: 'Check for updates' })).toBeDisabled()
  expect(remoteRequests).toEqual([])
})

test('automatic native checks run while the vault is locked and offer a verified release', async ({ page }) => {
  await mockProgramInfo(page, 'available')
  await page.goto('/')
  await expect.poll(() => checkCalls(page)).toBe(1)
  const about = await showAbout(page)
  await expect(about.getByText(`Version ${currentVersion}`, { exact: true })).toBeVisible()
  await expect(about.getByText('Linux · x86_64')).toBeVisible()
  await expect(about.getByText('Pre-release', { exact: true })).toBeVisible()
  await expect(about.getByText('Version 9.0.0 is available', { exact: true })).toBeVisible()
  await about.getByRole('button', { name: 'View release', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as unknown as { __VEYRA_ABOUT_TEST__: { calls: string[] } }).__VEYRA_ABOUT_TEST__.calls.includes('open_release_page'))).toBe(true)
})

test('automatic checks can be disabled while manual checking still works', async ({ page }) => {
  await mockProgramInfo(page, 'no_releases', false)
  await page.goto('/')
  const about = await showAbout(page)
  await expect(about.getByLabel('Automatically check for updates')).not.toBeChecked()
  expect(await checkCalls(page)).toBe(0)
  await about.getByRole('button', { name: 'Check for updates' }).click()
  await expect(about.getByText('No published releases yet', { exact: true })).toBeVisible()
  expect(await checkCalls(page)).toBe(1)
})

test('an update service failure is not reported as up to date', async ({ page }) => {
  await mockProgramInfo(page, 'failure')
  await page.goto('/')
  const about = await showAbout(page)
  await expect(about.getByText('Could not check for updates', { exact: true })).toBeVisible()
  await expect(about.getByText('Could not reach the release service securely.')).toBeVisible()
  await expect(about.getByText('You’re up to date', { exact: true })).toHaveCount(0)
  await expect(about.getByRole('button', { name: 'View release' })).toHaveCount(0)
})
