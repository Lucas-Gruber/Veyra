import type { Page } from '@playwright/test'
import type { ItemDraft, ManagedItemResult, ManagementOperation, VaultItem } from '../../../app/types/vault'

interface FixtureControls {
  calls: { command: string; args?: Record<string, unknown> }[]
  failNextSave: boolean
  delayNextEditor: boolean
  pendingEditor: boolean
  failBatchItem: string | null
  uncertainBatchItem: string | null
  delayNextChange: boolean
  pendingChange: boolean
  replaceAfterDelayedChange: boolean
  releaseEditor: () => void
  releaseChange: () => void
  replaceSession: () => void
  emitQuickBlur: () => void
}

declare global {
  interface Window { __VEYRA_UI_FIXTURE__: FixtureControls }
}

// Synthetic IPC contracts only. These tests do not authenticate, encrypt or contact
// Vaultwarden; the independently seeded Rust integration tests cover those paths.
export async function installConnectedVault(page: Page, folderFixture?: { folders: { id: string; name: string }[]; assignments: Record<string, string> }) {
  await page.addInitScript((folderFixture) => {
    let generation = 1
    let locked = false
    let needsSync = false
    let nextCallback = 0
    let nextItem = 0
    let releaseEditor: (() => void) | undefined
    let releaseChange: (() => void) | undefined
    const callbacks = new Map<number, (event: unknown) => void>()
    const listeners = new Map<string, number[]>()
    const organizations = [{ id: 'org-lantern', name: 'Lantern Guild' }, { id: 'org-river', name: 'River Cooperative' }, { id: 'org-empty', name: 'Empty group' }]
    const collections = [
      { id: 'coll-platform', organizationId: 'org-lantern', name: 'Platform team', readOnly: false, hidePasswords: false },
      { id: 'coll-finance', organizationId: 'org-lantern', name: 'Finance team', readOnly: true, hidePasswords: false },
      { id: 'coll-hidden', organizationId: 'org-lantern', name: 'Restricted team', readOnly: false, hidePasswords: true },
      { id: 'coll-river', organizationId: 'org-river', name: 'Operations', readOnly: false, hidePasswords: false },
    ]
    const definitions = [
      { id: 'personal-login', name: 'Personal fixture login', organizationId: null, collectionIds: [], kind: 'login', readOnly: false, deleted: false },
      { id: 'platform-login', name: 'Platform fixture login', organizationId: 'org-lantern', collectionIds: ['coll-platform'], kind: 'login', readOnly: false, deleted: false },
      { id: 'finance-login', name: 'Finance fixture login', organizationId: 'org-lantern', collectionIds: ['coll-finance'], kind: 'login', readOnly: true, deleted: false },
      { id: 'river-note', name: 'River fixture note', organizationId: 'org-river', collectionIds: ['coll-river'], kind: 'note', readOnly: false, deleted: false },
      { id: 'trashed-login', name: 'Trashed fixture login', organizationId: 'org-lantern', collectionIds: ['coll-platform'], kind: 'login', readOnly: false, deleted: true },
    ] as const
    const items: VaultItem[] = definitions.map(item => ({ ...item, collectionIds: [...item.collectionIds], favorite: false, username: item.kind === 'login' ? 'synthetic-user@example.test' : '', url: item.kind === 'login' ? 'https://example.test/' : '', domain: item.kind === 'login' ? 'example.test' : '', folder: '', color: 'violet', initials: 'SF', updated: '2026-10-29', tags: [], hasPassword: item.kind === 'login', hasTotp: false, revision: '2026-10-29T00:00:00.000Z', canDelete: !item.readOnly && !item.deleted, canRestore: item.deleted }))
    const folders = folderFixture?.folders || []
    for (const item of items) {
      item.folderId = folderFixture?.assignments[item.id] || null
      item.folder = folders.find(folder => folder.id === item.folderId)?.name || ''
    }
    const drafts: Record<string, ItemDraft> = Object.fromEntries(items.map(item => [item.id, { id: item.id, revision: item.revision || null, organizationId: item.organizationId || null, collectionIds: item.collectionIds || [], kind: item.kind, name: item.name, favorite: item.favorite, notes: 'Synthetic editor note', username: item.username, password: 'synthetic-editor-password', totp: '', uri: item.url, card: {}, identity: {} }]))
    const status = () => ({ locked, generation, mode: locked ? 'disconnected' : 'connected', lastSync: locked ? null : '2026-10-29T00:00:00Z' })
    const snapshot = () => structuredClone({ session: status(), items: locked ? [] : items, organizations: locked ? [] : organizations, collections: locked ? [] : collections, folders: locked ? [] : folders, unsupportedItems: 0, needsSync })
    const controls: FixtureControls = {
      calls: [], failNextSave: false, delayNextEditor: false, pendingEditor: false, failBatchItem: null, uncertainBatchItem: null,
      delayNextChange: false, pendingChange: false, replaceAfterDelayedChange: false,
      releaseEditor() { releaseEditor?.(); releaseEditor = undefined },
      releaseChange() { releaseChange?.(); releaseChange = undefined },
      replaceSession() {
        generation++; locked = false
        for (const id of listeners.get('vault:changed') || []) callbacks.get(id)?.({ payload: status() })
      },
      emitQuickBlur() { for (const id of listeners.get('quick-access:blur') || []) callbacks.get(id)?.({ payload: null }) },
    }
    Object.assign(window, {
      __VEYRA_UI_FIXTURE__: controls,
      isTauri: true,
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
      __TAURI_INTERNALS__: {
        transformCallback(callback: (event: unknown) => void) { callbacks.set(++nextCallback, callback); return nextCallback },
        unregisterCallback(id: number) { callbacks.delete(id) },
        async invoke(command: string, args?: Record<string, unknown>) {
          // Native IPC JSON-serializes arguments, including Vue reactive values.
          controls.calls.push({ command, args: args ? JSON.parse(JSON.stringify(args)) : undefined })
          if (command === 'plugin:event|listen') {
            const event = String(args?.event)
            listeners.set(event, [...(listeners.get(event) || []), Number(args?.handler)])
            return args?.handler
          }
          if (command === 'plugin:event|unlisten') return
          if (command === 'hide_quick_access') return
          if (command === 'open_website') return
          if (command === 'copy_field' || command === 'begin_browser_fill') return
          if (command === 'session_status') return status()
          if (command === 'list_metadata' || command === 'sync_vault') return snapshot()
          if (command === 'lock_vault') {
            locked = true; generation++
            for (const id of listeners.get('vault:locked') || []) callbacks.get(id)?.({ payload: status() })
            return status()
          }
          if (command === 'get_editable_item' || command === 'get_item_contents') {
            const result = { draft: structuredClone(drafts[String(args?.itemId)]), generation }
            if (controls.delayNextEditor) {
              controls.delayNextEditor = false; controls.pendingEditor = true
              await new Promise<void>(resolve => { releaseEditor = resolve })
              controls.pendingEditor = false
            }
            return result
          }
          if (command === 'save_item') {
            if (controls.failNextSave) { controls.failNextSave = false; throw { code: 'conflict', message: 'Synthetic conflict: synchronize before saving.' } }
            if (args?.expectedGeneration !== generation || locked) throw { code: 'locked', message: 'Vault is locked.' }
            const draft = structuredClone(args.draft) as ItemDraft
            const id = draft.id || `created-fixture-${++nextItem}`
            const previous = items.find(item => item.id === id)
            const item: VaultItem = { id, name: draft.name, username: draft.username, url: draft.uri, domain: draft.uri ? new URL(draft.uri).hostname : '', kind: draft.kind, favorite: draft.favorite, organizationId: draft.organizationId, collectionIds: draft.collectionIds, revision: '2026-10-29T00:01:00.000Z', deleted: false, readOnly: false, canDelete: true, canRestore: false, folder: '', color: 'violet', initials: 'SF', updated: '2026-10-29', tags: [], hasPassword: !!draft.password, hasTotp: !!draft.totp }
            if (previous) Object.assign(previous, item)
            else items.push(item)
            drafts[id] = { ...draft, id, revision: item.revision || null }
            return { itemId: id, refreshPending: false, snapshot: snapshot() }
          }
          if (command === 'change_item') {
            if (args?.expectedGeneration !== generation || locked) throw { code: 'locked', message: 'Vault is locked.' }
            const request = args.request as { id: string; action: string; favorite?: boolean }
            const item = items.find(item => item.id === request.id)
            if (!item) throw { code: 'not_found', message: 'Synthetic item not found.' }
            if (request.action === 'favorite') { item.favorite = !!request.favorite; drafts[item.id]!.favorite = item.favorite }
            else { item.deleted = request.action === 'trash'; item.canRestore = item.deleted; item.canDelete = !item.deleted }
            const result = { itemId: item.id, refreshPending: false, snapshot: snapshot() }
            if (controls.delayNextChange) {
              controls.delayNextChange = false; controls.pendingChange = true
              await new Promise<void>(resolve => { releaseChange = resolve })
              controls.pendingChange = false
              if (controls.replaceAfterDelayedChange) {
                controls.replaceAfterDelayedChange = false
                // Deliver the accepted old-session result first, then publish a
                // replacement before the dialog resumes its next batch item.
                queueMicrotask(() => queueMicrotask(() => controls.replaceSession()))
              }
            }
            return result
          }
          if (command === 'manage_items') {
            if (args?.expectedGeneration !== generation || locked || needsSync) throw { code: 'locked', message: 'Synthetic vault unavailable.' }
            const request = args.request as { items: { id: string; revision: string }[]; operation: ManagementOperation }
            const results: ManagedItemResult[] = []
            for (const source of request.items) {
              if (needsSync) { results.push({ sourceId: source.id, itemId: null, status: 'not_attempted', error: null }); continue }
              const item = items.find(item => item.id === source.id)
              if (!item || item.readOnly || controls.failBatchItem === source.id || controls.uncertainBatchItem === source.id) {
                needsSync = controls.uncertainBatchItem === source.id
                results.push({ sourceId: source.id, itemId: null, status: needsSync ? 'uncertain' : 'failed', error: { code: 'fixture_failure', message: needsSync ? 'Synthetic response lost. Synchronize before retrying.' : 'Synthetic item rejected.' } }); continue
              }
              let target = item
              const operation = request.operation
              if (operation.type === 'copy') {
                target = structuredClone(item); target.id = `copy-fixture-${++nextItem}`; target.name += ' (copy)'; items.push(target)
                drafts[target.id] = { ...structuredClone(drafts[item.id]!), id: target.id, name: target.name }
              }
              if (operation.type === 'copy' || operation.type === 'move') target.organizationId = operation.organizationId
              if ('collectionIds' in operation) target.collectionIds = [...operation.collectionIds]
              if ('folderId' in operation) { target.folderId = operation.folderId; target.folder = folders.find(folder => folder.id === operation.folderId)?.name || '' }
              results.push({ sourceId: source.id, itemId: target.id, status: 'succeeded', error: null })
            }
            return { results, refreshPending: needsSync, snapshot: snapshot() }
          }
          throw { code: 'not_supported', message: 'Unsupported synthetic UI fixture command.' }
        },
      },
    })
  }, folderFixture)
}
