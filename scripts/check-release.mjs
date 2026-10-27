import { readFileSync } from 'node:fs'

const root = new URL('../', import.meta.url)
const pkg = JSON.parse(readFileSync(new URL('package.json', root), 'utf8'))
const tauri = JSON.parse(readFileSync(new URL('src-tauri/tauri.conf.json', root), 'utf8'))
const cargo = readFileSync(new URL('src-tauri/Cargo.toml', root), 'utf8')
const packageSection = cargo.match(/\[package\]([\s\S]*?)(?=\n\[|$)/)?.[1]
const cargoVersion = packageSection?.match(/^version\s*=\s*"([^"]+)"/m)?.[1]
const semver = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/
if (!semver.test(pkg.version) || tauri.version !== pkg.version || cargoVersion !== pkg.version) {
  throw new Error('Package, Tauri and Cargo versions must be the same valid release version.')
}
const tag = process.env.RELEASE_TAG
if (tag && tag !== `v${pkg.version}`) throw new Error('Release tag does not match application version.')
if (pkg.license !== 'GPL-3.0-only') throw new Error('The combined SDK application must retain GPL-3.0-only licensing.')
console.log(`Validated Veyra ${pkg.version}${tag ? ` (${tag})` : ''}`)
