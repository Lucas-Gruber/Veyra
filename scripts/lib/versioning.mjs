import { accessSync, constants, readFileSync, renameSync, unlinkSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import semver from 'semver'

export const applicationId = 'io.github.LucasGruber.Veyra'
const metadataPath = `packaging/flatpak/${applicationId}.metainfo.xml`
const manifestPath = `packaging/flatpak/${applicationId}.json`
const desktopPath = `packaging/flatpak/${applicationId}.desktop`
const versionFiles = ['package.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/tauri.conf.json', metadataPath, manifestPath]

export function canonicalVersion(value) {
  if (typeof value !== 'string' || value.length > 128 || semver.valid(value) !== value || value.includes('+')) {
    throw new Error('Use a canonical SemVer version such as 1.2.3 or 1.2.3-alpha.1, without v prefix or build metadata.')
  }
  return value
}

function text(root, path) { return readFileSync(join(root, path), 'utf8') }
function json(content, path) {
  try { return JSON.parse(content) }
  catch { throw new Error(`Invalid JSON in ${path}.`) }
}
function stringField(section, field, path) {
  const matches = [...section.matchAll(new RegExp(`^${field}\\s*=\\s*"([^"\\r\\n]*)"\\s*(?:#.*)?$`, 'gm'))]
  if (matches.length !== 1) throw new Error(`Expected one quoted ${field} in ${path}.`)
  return matches[0][1]
}
function replaceString(section, field, value) {
  return section.replace(new RegExp(`^(${field}\\s*=\\s*")[^"\\r\\n]*("\\s*(?:#.*)?)$`, 'm'), (_, before, after) => `${before}${value}${after}`)
}
function cargoSection(content) {
  const matches = [...content.matchAll(/^\[package\][^\S\r\n]*(?:#.*)?\r?\n[\s\S]*?(?=^\[|(?![\s\S]))/gm)]
  if (matches.length !== 1) throw new Error('Expected one [package] section in src-tauri/Cargo.toml.')
  return matches[0][0]
}
function cargoLockSection(content, name) {
  const matches = [...content.matchAll(/^\[\[package\]\][^\S\r\n]*\r?\n[\s\S]*?(?=^\[\[package\]\]|(?![\s\S]))/gm)]
    .map(match => match[0]).filter(section => new RegExp(`^name\\s*=\\s*"${name}"\\s*$`, 'm').test(section) && !/^source\s*=/m.test(section))
  if (matches.length !== 1) throw new Error('Expected one local Veyra package in src-tauri/Cargo.lock.')
  return matches[0]
}
function attrs(tag) {
  return Object.fromEntries([...tag.matchAll(/([A-Za-z][\w-]*)\s*=\s*(["'])(.*?)\2/g)].map(match => [match[1], match[3]]))
}
function metadata(content) {
  const visible = content.replace(/<!--[\s\S]*?-->/g, comment => comment.replace(/[^\r\n]/g, ' '))
  const releases = [...visible.matchAll(/<release\b[^>]*>[\s\S]*?<\/release>/g)]
  if (!releases.length) throw new Error('AppStream metadata needs a current release entry.')
  const first = releases[0]
  return {
    id: visible.match(/<id>\s*([^<]+?)\s*<\/id>/)?.[1],
    license: visible.match(/<project_license>\s*([^<]+?)\s*<\/project_license>/)?.[1],
    desktop: visible.match(/<launchable\s+type=["']desktop-id["']>\s*([^<]+?)\s*<\/launchable>/)?.[1],
    currentTag: content.slice(first.index, first.index + first[0].indexOf('>') + 1),
    releases: releases.map(release => attrs(release[0].slice(0, release[0].indexOf('>') + 1))),
  }
}
function validDate(value) {
  if (typeof value !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(value) || Number.isNaN(Date.parse(`${value}T00:00:00Z`)) || new Date(`${value}T00:00:00Z`).toISOString().slice(0, 10) !== value) {
    throw new Error('Release date must be a real calendar date in YYYY-MM-DD format.')
  }
  return value
}
function readProject(root, overrides = {}) {
  const read = path => Object.hasOwn(overrides, path) ? overrides[path] : text(root, path)
  const contents = Object.fromEntries(versionFiles.map(path => [path, read(path)]))
  const pkg = json(contents['package.json'], 'package.json')
  const tauri = json(contents['src-tauri/tauri.conf.json'], 'src-tauri/tauri.conf.json')
  const cargo = cargoSection(contents['src-tauri/Cargo.toml'])
  if (pkg.name !== 'veyra' || stringField(cargo, 'name', 'Cargo.toml') !== 'veyra') throw new Error('Expected the Veyra project identity.')
  const lock = cargoLockSection(contents['src-tauri/Cargo.lock'], 'veyra')
  const appstream = metadata(contents[metadataPath])
  const flatpak = json(read(manifestPath), manifestPath)
  const desktop = read(desktopPath)
  return { contents, pkg, tauri, cargo, lock, appstream, flatpak, desktop }
}
function validateIdentity(project) {
  const { pkg, tauri, cargo, appstream, flatpak, desktop } = project
  if (pkg.license !== 'GPL-3.0-only' || stringField(cargo, 'license', 'Cargo.toml') !== pkg.license || appstream.license !== pkg.license) throw new Error('Veyra package, Cargo and AppStream licenses must remain GPL-3.0-only.')
  if (tauri.identifier !== applicationId || flatpak.id !== applicationId || appstream.id !== applicationId || appstream.desktop !== `${applicationId}.desktop` || !new RegExp(`^Icon=${applicationId.replaceAll('.', '\\.')}\\s*$`, 'm').test(desktop)) throw new Error('Tauri, Flatpak, AppStream and desktop application IDs must match.')
}

function checkLoadedProject(project, tag) {
  validateIdentity(project)
  const version = canonicalVersion(project.pkg.version)
  const versions = [project.tauri.version, stringField(project.cargo, 'version', 'Cargo.toml'), stringField(project.lock, 'version', 'Cargo.lock'), project.appstream.releases[0].version]
  if (versions.some(value => value !== version)) throw new Error('Version drift: package.json, Cargo manifest/lock, Tauri and current AppStream release must match. Run pnpm version:set VERSION.')
  const releaseType = semver.prerelease(version) ? 'development' : 'stable'
  if (project.flatpak['default-branch'] !== (releaseType === 'development' ? 'beta' : 'stable')) throw new Error('Flatpak branch must match the stable/prerelease version channel.')
  if (project.appstream.releases[0].type !== releaseType) throw new Error('AppStream release type must match the stable/prerelease version channel.')
  validDate(project.appstream.releases[0].date)
  const history = project.appstream.releases.map(release => canonicalVersion(release.version))
  if (new Set(history).size !== history.length) throw new Error('AppStream release history contains duplicate versions.')
  if (tag !== undefined && tag !== '' && tag !== `v${version}`) throw new Error('Release tag does not match the application version.')
  return { version, channel: releaseType === 'stable' ? 'stable' : 'prerelease', tag: `v${version}`, applicationId }
}

export function checkProjectVersion(root, { tag } = {}) {
  return checkLoadedProject(readProject(root), tag)
}

export function setProjectVersion(root, value, { date, allowUnreleasedDowngrade = false } = {}) {
  const version = canonicalVersion(value)
  const project = readProject(root)
  validateIdentity(project)
  const previous = canonicalVersion(project.pkg.version)
  if (semver.lt(version, previous) && !allowUnreleasedDowngrade) throw new Error('Refusing a version downgrade. Only an unreleased draft migration may use --allow-unreleased-downgrade.')
  const currentRelease = project.appstream.releases[0]
  if (currentRelease.version !== version && project.appstream.releases.slice(1).some(release => release.version === version)) throw new Error('The requested version already exists in AppStream history; choose a new version.')
  const releaseDate = validDate(date ?? (currentRelease.version === version ? currentRelease.date : new Date().toISOString().slice(0, 10)))
  const type = semver.prerelease(version) ? 'development' : 'stable'
  project.pkg.version = version
  project.tauri.version = version
  project.flatpak['default-branch'] = type === 'development' ? 'beta' : 'stable'
  const updates = {
    'package.json': `${JSON.stringify(project.pkg, null, 2)}\n`,
    [manifestPath]: `${JSON.stringify(project.flatpak, null, 2)}\n`,
    'src-tauri/tauri.conf.json': `${JSON.stringify(project.tauri, null, 2)}\n`,
    'src-tauri/Cargo.toml': project.contents['src-tauri/Cargo.toml'].replace(project.cargo, replaceString(project.cargo, 'version', version)),
    'src-tauri/Cargo.lock': project.contents['src-tauri/Cargo.lock'].replace(project.lock, replaceString(project.lock, 'version', version)),
  }
  const xml = project.contents[metadataPath]
  if (currentRelease.version === version) {
    const tag = project.appstream.currentTag.replace(/\b(date|type)\s*=\s*(["'])(.*?)\2/g, (_, attr, quote) => `${attr}=${quote}${attr === 'date' ? releaseDate : type}${quote}`)
    if (!/\bdate\s*=/.test(tag) || !/\btype\s*=/.test(tag)) throw new Error('The current AppStream release needs date and type attributes.')
    updates[metadataPath] = xml.replace(project.appstream.currentTag, tag)
  } else {
    const entry = `    <release version="${version}" date="${releaseDate}" type="${type}">\n      <description><p>${type === 'development' ? 'Development preview.' : `Veyra ${version}.`} See project documentation for verified features and compatibility limitations.</p></description>\n    </release>`
    if (!/<releases\b[^>]*>/.test(xml)) throw new Error('Missing AppStream releases container.')
    updates[metadataPath] = xml.replace(/(<releases\b[^>]*>)(?:\r?\n)?/, `$1\n${entry}\n`)
  }
  const identity = checkLoadedProject(readProject(root, updates))
  // Parse and validate every source before touching disk. Temporary files are
  // staged first, so malformed input or an unwritable target leaves originals intact.
  const temporary = []
  let failure
  try {
    for (const path of Object.keys(updates)) accessSync(join(root, path), constants.W_OK)
    for (const [path, content] of Object.entries(updates)) {
      const pending = `${join(root, path)}.version-${process.pid}.tmp`
      writeFileSync(pending, content, { flag: 'wx' })
      temporary.push([pending, join(root, path)])
    }
    for (const [pending, target] of temporary) renameSync(pending, target)
  } catch (error) { failure = error }
  for (const [pending] of temporary) {
    try { unlinkSync(pending) } catch (error) { if (error.code !== 'ENOENT') failure ??= error }
  }
  if (failure) throw failure
  return identity
}
