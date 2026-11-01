import assert from 'node:assert/strict'
import { mkdtemp, mkdir, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import vm from 'node:vm'
import { test } from 'node:test'
import { chromium } from '@playwright/test'
import { parseArguments, nativeManifest, run } from '../../scripts/browser-companion.mjs'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const fillSource = await readFile(join(root, 'browser-companion/fill.js'), 'utf8')
const id = '49867d4a-8931-424f-888c-2d0cdd832d92'

test('marker parsing requires HTTPS and preserves the entire target', () => {
  const context = vm.createContext({ URL })
  vm.runInContext(fillSource, context)
  const parse = context.VeyraFill.parseTarget
  assert.equal(parse(`https://example.test/login?q=1#route&__veyra_fill=${id}`).targetUrl, 'https://example.test/login?q=1#route')
  assert.equal(parse(`https://example.test/login#__veyra_fill=${id}`).intentId, id)
  assert.equal(parse(`http://example.test/login#__veyra_fill=${id}`), null)
  assert.equal(parse(`https://user:pass@example.test/login#__veyra_fill=${id}`), null)
  assert.equal(parse('https://example.test/#__veyra_fill=not-an-intent'), null)
  assert.equal(parse(`https://example.test/#__veyra_fill=${id}&other=value`), null)
})

test('background verifies extension, top-frame sender, full URL and request marker', async () => {
  let listener
  const forwarded = []
  const api = { runtime: { id: 'own-id', onMessage: { addListener(fn) { listener = fn } }, async sendNativeMessage(_host, message) { forwarded.push(message); return { status: 'ready' } } } }
  const context = vm.createContext({ URL, chrome: api })
  vm.runInContext(await readFile(join(root, 'browser-companion/background.js'), 'utf8'), context)
  await Promise.resolve()
  forwarded.length = 0
  const sender = { id: 'own-id', tab: { id: 3 }, frameId: 0, url: `https://example.test/login#__veyra_fill=${id}` }
  const message = { type: 'veyra_take', intentId: id, targetUrl: 'https://example.test/login' }
  for (const malicious of [ { ...sender, id: 'other' }, { ...sender, frameId: 1 }, { ...sender, url: `https://evil.test/login#__veyra_fill=${id}` }, { ...sender, url: 'https://example.test/login' } ]) {
    assert.equal(listener(message, malicious, () => {}), false)
  }
  assert.equal(forwarded.length, 0)
  assert.equal(listener(message, sender, () => {}), true)
  await Promise.resolve()
  assert.equal(forwarded.length, 1)
  assert.equal(forwarded[0].target_url, 'https://example.test/login')
})

test('installer prepares sources and only writes exact native allowlists when applied', async () => {
  const temporary = await mkdtemp(join(tmpdir(), 'veyra-companion-test-'))
  try {
    const environment = { HOME: temporary, XDG_CONFIG_HOME: join(temporary, '.config') }
    const host = join(temporary, 'veyra-native-host')
    await writeFile(host, '#!/bin/sh\nexit 0\n', { mode: 0o700 })
    const options = parseArguments(['--output', join(temporary, 'extension'), '--host', host])
    const prepared = await run(options, environment)
    assert.equal(JSON.parse(await readFile(join(prepared.extensionDirectory, 'manifest.json'), 'utf8')).background.scripts[0], 'background.js')
    await assert.rejects(stat(prepared.manifest), { code: 'ENOENT' })
    const applied = await run({ ...options, apply: true }, environment)
    assert.deepEqual(JSON.parse(await readFile(applied.manifest, 'utf8')).allowed_extensions, ['companion@veyra.app'])
    assert.equal((await stat(applied.manifest)).mode & 0o777, 0o600)
    assert.throws(() => parseArguments(['--browser', 'chromium', '--apply']), /chromium-id/)
    assert.throws(() => parseArguments(['--browser', 'chromium', '--chromium-id', '*']), /32 letters/)
    assert.deepEqual(nativeManifest({ host, browser: 'chromium', chromiumId: 'abcdefghijklmnopabcdefghijklmnop' }).allowed_origins, ['chrome-extension://abcdefghijklmnopabcdefghijklmnop/'])
    const unsafe = join(temporary, 'unsafe')
    await mkdir(unsafe, { mode: 0o755 })
    await mkdir(join(unsafe, 'veyra'), { mode: 0o755 })
    await assert.rejects(run({ ...options, apply: true }, { HOME: temporary, XDG_CONFIG_HOME: unsafe }), /private user-owned directory/)
  } finally { await rm(temporary, { recursive: true, force: true }) }
})

test('browser DOM fill targets a single visible same-origin login and never submits', async () => {
  const browser = await chromium.launch({ headless: true })
  try {
    const page = await browser.newPage()
    await page.route('https://example.test/**', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><title>Login fixture</title>' }))
    await page.goto('https://example.test/login?q=1#route')
    const credentials = { username: 'fixture@example.test', password: 'synthetic-browser-test-password' }
    const execute = async (html, target = page.url()) => {
      await page.setContent(html)
      await page.addScriptTag({ content: fillSource })
      return page.evaluate(({ credentials, target }) => globalThis.VeyraFill.fillLogin(credentials, target), { credentials, target })
    }
    assert.equal(await execute('<form action="/session"><input autocomplete="username"><input type="password" autocomplete="current-password"><button>Sign in</button></form><script>globalThis.submitted=false;document.querySelector("form").onsubmit=()=>{globalThis.submitted=true;return false}</script>'), true)
    assert.equal(await page.locator('input[type=password]').inputValue(), credentials.password)
    assert.equal(await page.evaluate(() => globalThis.submitted), false)
    for (const html of [
      '<form action="https://evil.test/steal"><input autocomplete="username"><input type="password"></form>',
      '<form><input autocomplete="username"><input type="password"><input type="password"></form>',
      '<form><input autocomplete="username"><input type="password" autocomplete="new-password"></form>',
      '<form><input autocomplete="username"><input type="password" style="display:none"></form>',
      '<form><input autocomplete="username"><input type="password" readonly></form>',
    ]) {
      assert.equal(await execute(html), false)
      for (const value of await page.locator('input').evaluateAll(inputs => inputs.map(input => input.value))) assert.equal(value, '')
    }
    assert.equal(await execute('<form><input autocomplete="username"><input type="password"></form>', 'https://example.test/other'), false)
  } finally { await browser.close() }
})
