#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-only
import { constants } from 'node:fs'
import { access, chmod, copyFile, lstat, mkdir, readFile, realpath, rename, writeFile } from 'node:fs/promises'
import { dirname, isAbsolute, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const project = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const hostName = 'app.veyra.companion'

export function parseArguments(args) {
  const options = { browser: 'firefox', chromiumId: null, host: null, apply: false, output: join(project, 'dist/browser-companion') }
  for (let index = 0; index < args.length; index++) {
    const arg = args[index]
    if (arg === '--apply') { options.apply = true; continue }
    if (!['--browser', '--chromium-id', '--host', '--output'].includes(arg) || !args[index + 1]) throw new Error(`Unknown or incomplete argument: ${arg}`)
    const value = args[++index]
    if (arg === '--browser') options.browser = value
    if (arg === '--chromium-id') options.chromiumId = value
    if (arg === '--host') options.host = value
    if (arg === '--output') options.output = resolve(value)
  }
  if (!['firefox', 'chromium', 'chrome'].includes(options.browser)) throw new Error('Browser must be firefox, chromium or chrome.')
  if (options.chromiumId && !/^[a-p]{32}$/.test(options.chromiumId)) throw new Error('Chromium extension ID must contain exactly 32 letters a-p.')
  if (options.apply && options.browser !== 'firefox' && !options.chromiumId) throw new Error('Pass the ID displayed by chrome://extensions using --chromium-id.')
  if (options.host && !isAbsolute(options.host)) throw new Error('--host must be an absolute path.')
  if (options.apply && !options.host) throw new Error('--apply requires --host pointing to the built veyra-native-host binary.')
  return options
}

export function nativeManifest(options) {
  return {
    name: hostName,
    description: 'Veyra explicit single-use browser filling',
    path: options.host,
    type: 'stdio',
    ...(options.browser === 'firefox'
      ? { allowed_extensions: ['companion@veyra.app'] }
      : { allowed_origins: [`chrome-extension://${options.chromiumId}/`] }),
  }
}

async function privateDirectory(path) {
  await mkdir(path, { recursive: true, mode: 0o700 })
  const info = await lstat(path)
  if (!info.isDirectory() || info.uid !== process.getuid() || (info.mode & 0o077) !== 0) throw new Error(`Expected a private user-owned directory: ${path}`)
}

async function writePrivate(path, data) {
  try {
    const info = await lstat(path)
    if (!info.isFile() || info.uid !== process.getuid()) throw new Error(`Refusing to replace a non-regular or foreign file: ${path}`)
  } catch (error) { if (error.code !== 'ENOENT') throw error }
  const temporary = `${path}.veyra-${process.pid}`
  await writeFile(temporary, data, { mode: 0o600, flag: 'wx' })
  await rename(temporary, path)
  await chmod(path, 0o600)
}

export async function run(options, environment = process.env) {
  if (process.platform !== 'linux') throw new Error('The native bridge currently supports Linux only.')
  const home = environment.HOME
  if (!home || !isAbsolute(home)) throw new Error('An absolute HOME directory is required.')
  const config = environment.XDG_CONFIG_HOME || join(home, '.config')
  if (!isAbsolute(config)) throw new Error('XDG_CONFIG_HOME must be absolute.')
  const manifestDirectory = options.browser === 'firefox'
    ? join(home, '.mozilla/native-messaging-hosts')
    : join(config, options.browser === 'chrome' ? 'google-chrome' : 'chromium', 'NativeMessagingHosts')
  const plan = {
    extensionDirectory: join(options.output, options.browser === 'firefox' ? 'firefox' : 'chromium'),
    manifest: join(manifestDirectory, `${hostName}.json`),
    host: options.host,
    browser: options.browser,
    apply: options.apply,
  }
  // Preparing extension source is reversible and never modifies a browser profile.
  await mkdir(plan.extensionDirectory, { recursive: true })
  for (const file of ['background.js', 'fill.js', 'content.js']) await copyFile(join(project, 'browser-companion', file), join(plan.extensionDirectory, file))
  await copyFile(join(project, 'browser-companion', options.browser === 'firefox' ? 'manifest.firefox.json' : 'manifest.json'), join(plan.extensionDirectory, 'manifest.json'))
  if (!options.apply) return plan
  await access(options.host, constants.X_OK)
  options = { ...options, host: await realpath(options.host) }
  const info = await lstat(options.host)
  if (!info.isFile()) throw new Error('--host must resolve to a regular executable.')
  await privateDirectory(join(config, 'veyra'))
  const configFile = join(config, 'veyra/browser-companion.json')
  let ids = []
  try {
    const previous = await lstat(configFile)
    if (!previous.isFile() || previous.uid !== process.getuid() || (previous.mode & 0o077) !== 0 || previous.size > 16384) throw new Error('Existing companion configuration must be a private user-owned regular file.')
    const data = JSON.parse(await readFile(configFile, 'utf8'))
    if (!Array.isArray(data.chromium_extension_ids) || !data.chromium_extension_ids.every(id => /^[a-p]{32}$/.test(id))) throw new Error('Invalid existing Chromium allowlist.')
    ids = data.chromium_extension_ids
  } catch (error) { if (error.code !== 'ENOENT') throw error }
  if (options.chromiumId && !ids.includes(options.chromiumId)) ids.push(options.chromiumId)
  await writePrivate(configFile, `${JSON.stringify({ chromium_extension_ids: ids }, null, 2)}\n`)
  await privateDirectory(manifestDirectory)
  await writePrivate(plan.manifest, `${JSON.stringify(nativeManifest(options), null, 2)}\n`)
  return { ...plan, host: options.host }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { console.log(JSON.stringify(await run(parseArguments(process.argv.slice(2))), null, 2)) }
  catch (error) { console.error(error.message); process.exitCode = 1 }
}
