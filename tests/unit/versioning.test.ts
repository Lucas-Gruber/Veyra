import { afterEach, describe, expect, it } from 'vitest'
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { applicationId, canonicalVersion, checkProjectVersion, setProjectVersion } from '../../scripts/lib/versioning.mjs'

const roots: string[] = []
const version = '1.2.3'
const sources = () => ({
  'package.json': JSON.stringify({ name: 'veyra', version, license: 'GPL-3.0-only' }),
  'src-tauri/Cargo.toml': `[package]\nname = "veyra"\nversion = "${version}"\nlicense = "GPL-3.0-only"\n\n[dependencies]\nother = "4.5.6"\n`,
  'src-tauri/Cargo.lock': `version = 4\n\n[[package]]\nname = "other"\nversion = "7.8.9"\nsource = "registry+https://example.test"\n\n[[package]]\nname = "veyra"\nversion = "${version}"\ndependencies = ["other"]\n`,
  'src-tauri/tauri.conf.json': JSON.stringify({ identifier: applicationId, version }),
  [`packaging/flatpak/${applicationId}.json`]: JSON.stringify({ id: applicationId, 'runtime-version': '51', 'default-branch': 'stable' }),
  [`packaging/flatpak/${applicationId}.desktop`]: `[Desktop Entry]\nVersion=1.0\nIcon=${applicationId}\n`,
  [`packaging/flatpak/${applicationId}.metainfo.xml`]: `<component><id>${applicationId}</id><project_license>GPL-3.0-only</project_license><launchable type="desktop-id">${applicationId}.desktop</launchable><releases>\n    <release version="${version}" date="2026-10-01" type="stable"><description><p>Existing notes</p></description></release>\n    <release version="1.2.2" date="2026-09-01" type="stable"><description><p>Historical notes</p></description></release>\n</releases></component>\n`,
})
function fixture() {
  const root = mkdtempSync(join(tmpdir(), 'veyra-versioning-'))
  roots.push(root)
  for (const [path, content] of Object.entries(sources())) {
    mkdirSync(dirname(join(root, path)), { recursive: true })
    writeFileSync(join(root, path), content)
  }
  return root
}
function snapshot(root: string) { return Object.fromEntries(Object.keys(sources()).map(path => [path, readFileSync(join(root, path), 'utf8')])) }
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }) })

describe('canonical release identity', () => {
  it.each(['1.2.3', '0.1.0-alpha.1', '1.2.3-rc.10'])('accepts %s', value => expect(canonicalVersion(value)).toBe(value))
  it.each(['v1.2.3', '01.2.3', '1.2.3-01', '1.2', ' 1.2.3', '1.2.3 ', '1.2.3+build.1', '1.2.3-'])('rejects noncanonical release version %s', value => expect(() => canonicalVersion(value)).toThrow())
  it('checks every release manifest and matching tag', () => {
    expect(checkProjectVersion(fixture(), { tag: 'v1.2.3' })).toMatchObject({ version, channel: 'stable', tag: 'v1.2.3' })
  })
  it('rejects a delivery branch that would offer prereleases on the stable channel', () => {
    const root = fixture(), path = join(root, `packaging/flatpak/${applicationId}.json`)
    const manifest = JSON.parse(readFileSync(path, 'utf8'))
    manifest['default-branch'] = 'beta'
    writeFileSync(path, JSON.stringify(manifest))
    expect(() => checkProjectVersion(root)).toThrow('Flatpak branch')
  })
  it('rejects a stale or mismatched release tag', () => expect(() => checkProjectVersion(fixture(), { tag: 'v1.2.2' })).toThrow('tag'))
  it.each(['src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/tauri.conf.json', `packaging/flatpak/${applicationId}.metainfo.xml`])('rejects drift in %s', path => {
    const root = fixture()
    writeFileSync(join(root, path), readFileSync(join(root, path), 'utf8').replace('1.2.3', '1.2.4'))
    expect(() => checkProjectVersion(root)).toThrow('drift')
  })
  it('rejects application ID drift', () => {
    const root = fixture(), path = join(root, 'src-tauri/tauri.conf.json')
    writeFileSync(path, readFileSync(path, 'utf8').replace(applicationId, 'io.example.Other'))
    expect(() => checkProjectVersion(root)).toThrow('IDs')
  })
})

describe('coordinated version changes', () => {
  it('updates all manifests together, keeps history and leaves dependency/runtime/spec versions alone', () => {
    const root = fixture()
    expect(setProjectVersion(root, '1.3.0-alpha.1', { date: '2026-10-27' })).toMatchObject({ version: '1.3.0-alpha.1', channel: 'prerelease' })
    expect(checkProjectVersion(root, { tag: 'v1.3.0-alpha.1' }).version).toBe('1.3.0-alpha.1')
    expect(readFileSync(join(root, 'src-tauri/Cargo.lock'), 'utf8')).toContain('version = "7.8.9"')
    expect(readFileSync(join(root, `packaging/flatpak/${applicationId}.desktop`), 'utf8')).toContain('Version=1.0')
    const flatpak = JSON.parse(readFileSync(join(root, `packaging/flatpak/${applicationId}.json`), 'utf8'))
    expect(flatpak['runtime-version']).toBe('51')
    expect(flatpak['default-branch']).toBe('beta')
    const xml = readFileSync(join(root, `packaging/flatpak/${applicationId}.metainfo.xml`), 'utf8')
    expect(xml).toContain('version="1.3.0-alpha.1" date="2026-10-27" type="development"')
    expect(xml).toContain('Existing notes')
    expect(xml).toContain('Historical notes')
  })
  it('is repeatable without adding duplicate history or changing the release date', () => {
    const root = fixture()
    setProjectVersion(root, '1.3.0-alpha.1', { date: '2026-10-27' })
    const before = snapshot(root)
    setProjectVersion(root, '1.3.0-alpha.1')
    expect(snapshot(root)).toEqual(before)
  })
  it('turns a candidate into a stable version with stable AppStream metadata', () => {
    const root = fixture()
    setProjectVersion(root, '1.3.0-rc.1', { date: '2026-10-27' })
    expect(setProjectVersion(root, '1.3.0', { date: '2026-10-28' }).channel).toBe('stable')
    expect(checkProjectVersion(root).channel).toBe('stable')
    expect(JSON.parse(readFileSync(join(root, `packaging/flatpak/${applicationId}.json`), 'utf8'))['default-branch']).toBe('stable')
  })
  it('rejects invalid versions and dates before modifying any file', () => {
    const root = fixture(), before = snapshot(root)
    expect(() => setProjectVersion(root, '1.3.0-01')).toThrow()
    expect(() => setProjectVersion(root, '1.3.0', { date: '2026-02-30' })).toThrow('date')
    expect(snapshot(root)).toEqual(before)
  })
  it('rejects malformed source/history before any write', () => {
    const root = fixture(), path = join(root, `packaging/flatpak/${applicationId}.metainfo.xml`)
    writeFileSync(path, readFileSync(path, 'utf8').replace('1.2.2', 'not-a-version'))
    const before = snapshot(root)
    expect(() => setProjectVersion(root, '1.3.0')).toThrow()
    expect(snapshot(root)).toEqual(before)
  })
  it('prevents accidental downgrade unless the unreleased migration is explicit', () => {
    const root = fixture(), before = snapshot(root)
    expect(() => setProjectVersion(root, '1.2.3-alpha.1')).toThrow('downgrade')
    expect(snapshot(root)).toEqual(before)
    expect(setProjectVersion(root, '1.2.3-alpha.1', { allowUnreleasedDowngrade: true }).channel).toBe('prerelease')
  })
})
