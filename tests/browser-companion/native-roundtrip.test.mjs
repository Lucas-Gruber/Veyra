import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { chmod, copyFile, mkdir, mkdtemp, rm, stat } from 'node:fs/promises'
import { createServer } from 'node:net'
import { endianness, tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { promisify } from 'node:util'
import { test } from 'node:test'
import { chromium } from '@playwright/test'
import { parseArguments, run } from '../../scripts/browser-companion.mjs'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const host = process.env.VEYRA_TEST_NATIVE_HOST || join(root, 'src-tauri/target/debug/veyra-native-host')
const exists = await stat(host).then(info => info.isFile(), () => false)

test('installed Chromium companion talks through the real Rust native host and confirms DOM filling', { skip: !exists ? 'Build veyra-native-host first to enable the isolated native round trip.' : false, timeout: 20000 }, async () => {
  const temporary = await mkdtemp(join(tmpdir(), 'veyra-browser-'))
  let context
  let server
  const connections = new Set()
  try {
    const configuration = join(temporary, 'config')
    const runtime = join(temporary, 'run')
    await mkdir(configuration, { mode: 0o700 })
    await mkdir(runtime, { mode: 0o700 })
    await mkdir(join(runtime, 'veyra-browser'), { mode: 0o700 })
    const environment = { ...process.env, HOME: temporary, XDG_CONFIG_HOME: configuration, XDG_RUNTIME_DIR: runtime }
    const output = join(temporary, 'extension')
    const options = parseArguments(['--browser', 'chromium', '--host', host, '--output', output])
    const prepared = await run(options, environment)
    const requests = []
    const intentId = '49867d4a-8931-424f-888c-2d0cdd832d92'
    const targetUrl = 'https://example.test/login?q=1#route'
    const credentials = { username: 'fixture@example.test', password: 'synthetic-native-roundtrip-password' }
    let delivered = false
    let acknowledge
    const acknowledged = new Promise(resolve => { acknowledge = resolve })
    server = createServer(stream => {
      connections.add(stream)
      stream.on('close', () => connections.delete(stream))
      let bytes = Buffer.alloc(0)
      stream.on('data', chunk => {
        bytes = Buffer.concat([bytes, chunk])
        if (bytes.length < 4) return
        const length = endianness() === 'LE' ? bytes.readUInt32LE(0) : bytes.readUInt32BE(0)
        if (length > 16384) { stream.destroy(); return }
        if (bytes.length < 4 + length) return
        const request = JSON.parse(bytes.subarray(4, 4 + length))
        requests.push(request)
        let response = { status: 'rejected' }
        if (request.type === 'hello') response = { status: 'ready' }
        if (request.type === 'take' && request.intent_id === intentId && request.target_url === targetUrl && !delivered) {
          delivered = true
          response = { status: 'credentials', ...credentials }
        }
        if (request.type === 'ack' && request.intent_id === intentId && request.target_url === targetUrl) {
          response = { status: 'done' }
          acknowledge(request)
        }
        const payload = Buffer.from(JSON.stringify(response))
        const header = Buffer.alloc(4)
        if (endianness() === 'LE') header.writeUInt32LE(payload.length)
        else header.writeUInt32BE(payload.length)
        stream.end(Buffer.concat([header, payload]))
      })
    })
    const socket = join(runtime, 'veyra-browser/bridge.sock')
    await new Promise((resolve, reject) => { server.once('error', reject); server.listen(socket, resolve) })
    await chmod(socket, 0o600)
    context = await chromium.launchPersistentContext(join(temporary, 'profile'), {
      headless: true,
      channel: 'chromium',
      env: environment,
      args: [`--disable-extensions-except=${prepared.extensionDirectory}`, `--load-extension=${prepared.extensionDirectory}`],
    })
    const worker = context.serviceWorkers()[0] || await context.waitForEvent('serviceworker', { timeout: 5000 })
    const extensionId = new URL(worker.url()).hostname
    const installed = await run({ ...options, chromiumId: extensionId, apply: true }, environment)
    // Playwright's Chromium may use Chrome-for-Testing's native-host directory.
    for (const browser of ['google-chrome-for-testing', 'google-chrome']) {
      const directory = join(configuration, browser, 'NativeMessagingHosts')
      await mkdir(directory, { recursive: true, mode: 0o700 })
      await copyFile(installed.manifest, join(directory, 'app.veyra.companion.json'))
    }
    // --user-data-dir also overrides Chromium's per-user native-host directory.
    const profileHosts = join(temporary, 'profile', 'NativeMessagingHosts')
    await mkdir(profileHosts, { recursive: true, mode: 0o700 })
    await copyFile(installed.manifest, join(profileHosts, 'app.veyra.companion.json'))
    const handshake = await worker.evaluate(async () => {
      try { return await globalThis.chrome.runtime.sendNativeMessage('app.veyra.companion', { type: 'hello' }) }
      catch (error) { return { status: 'error', message: error.message } }
    })
    assert.equal(handshake.status, 'ready', handshake.message)
    await assert.rejects(promisify(execFile)(host, ['chrome-extension://not-allowed/'], { env: environment }), error => error.code === 1 && error.stdout === '')
    const page = await context.newPage()
    await page.route('https://example.test/**', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><form action="/session"><input autocomplete="username"><input type="password" autocomplete="current-password"><button>Sign in</button></form>' }))
    await page.goto(`${targetUrl}&__veyra_fill=${intentId}`)
    const ack = await Promise.race([acknowledged, new Promise((_, reject) => setTimeout(() => reject(new Error(`Native fill was not acknowledged; request types: ${requests.map(request => request.type).join(', ')}`)), 8000).unref())])
    assert.equal(ack.filled, true)
    assert.equal(await page.locator('input[type=password]').inputValue(), credentials.password)
    assert.equal(await page.locator('input[autocomplete=username]').inputValue(), credentials.username)
    assert.equal(page.url(), targetUrl)
    assert.equal(requests.filter(request => request.type === 'take').length, 1)
    // Page scripts cannot access the isolated-world helper or extension API.
    assert.equal(await page.evaluate(() => typeof globalThis.VeyraFill), 'undefined')
  } finally {
    if (context) await context.close()
    for (const connection of connections) connection.destroy()
    if (server) await new Promise(resolve => server.close(resolve))
    await rm(temporary, { recursive: true, force: true })
  }
})
