import type { Page } from '@playwright/test'
import type { ItemDraft, VaultItem } from '../../../app/types/vault'

interface FixtureControls {
  calls: { command: string; args?: Record<string, unknown> }[]
  failNextSave: boolean
  delayNextEditor: boolean
  pendingEditor: boolean
  releaseEditor: () => void
  emitQuickBlur: () => void
}

declare global {
  interface Window { __VEYRA_UI_FIXTURE__: FixtureControls }
}

// Synthetic IPC contracts only. These tests do not authenticate, encrypt or contact
// Vaultwarden; the independently seeded Rust integration tests cover those paths.
export async function installConnectedVault(page: Page) {
  await page.addInitScript(() => {
    let generation = 1
    let locked = false
    let nextCallback = 0
    let nextItem = 0
    let releaseEditor: (() => void) | undefined
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
    const drafts: Record<string, ItemDraft> = Object.fromEntries(items.map(item => [item.id, { id: item.id, revision: item.revision || null, organizationId: item.organizationId || null, collectionIds: item.collectionIds || [], kind: item.kind, name: item.name, favorite: item.favorite, notes: 'Synthetic editor note', username: item.username, password: 'synthetic-editor-password', totp: '', uri: item.url, card: {}, identity: {} }]))
    const status = () => ({ locked, generation, mode: locked ? 'disconnected' : 'connected', lastSync: locked ? null : '2026-10-29T00:00:00Z' })
    const snapshot = () => structuredClone({ session: status(), items: locked ? [] : items, organizations: locked ? [] : organizations, collections: locked ? [] : collections, unsupportedItems: 0, needsSync: false })
    const controls: FixtureControls = {
      calls: [], failNextSave: false, delayNextEditor: false, pendingEditor: false,
      releaseEditor() { releaseEditor?.(); releaseEditor = undefined },
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
          controls.calls.push({ command, args: args ? structuredClone(args) : undefined })
          if (command === 'plugin:event|listen') {
            const event = String(args?.event)
            listeners.set(event, [...(listeners.get(event) || []), Number(args?.handler)])
            return args?.handler
          }
          if (command === 'plugin:event|unlisten') return
          if (command === 'hide_quick_access') return
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
            return { itemId: item.id, refreshPending: false, snapshot: snapshot() }
          }
          throw { code: 'not_supported', message: 'Unsupported synthetic UI fixture command.' }
        },
      },
    })
  })
}
