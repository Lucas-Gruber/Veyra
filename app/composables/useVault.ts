import { isTauri } from '@tauri-apps/api/core'
import { demoItems } from '~/data/demo'
import type { CopyField, LoginOutcome, LoginRequest, SessionStatus, VaultSnapshot, VaultItem } from '~/types/vault'

export function useVault() {
  const session = useState<SessionStatus>('vault:session', () => ({ locked: true, mode: 'disconnected', generation: 0, lastSync: null }))
  const locked = computed(() => session.value.locked)
  const demo = computed(() => session.value.mode === 'demo')
  const connected = computed(() => session.value.mode === 'connected' && !session.value.locked)
  const items = useState<VaultItem[]>('vault:items', () => [])
  const recent = useState<string[]>('vault:recent', () => [])
  const feedback = useState('vault:feedback', () => '')
  const syncing = useState('vault:syncing', () => false)
  const unsupportedItems = useState('vault:unsupported-items', () => 0)
  const desktop = computed(() => import.meta.client && isTauri())
  const sessionEpoch = useState('vault:session-epoch', () => 0)
  let toastTimer: ReturnType<typeof setTimeout> | undefined
  function notify(message: string) {
    feedback.value = message
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => { feedback.value = '' }, 4500)
  }
  function errorMessage(error: unknown, fallback: string): string {
    if (error && typeof error === 'object' && 'message' in error && typeof error.message === 'string') return error.message
    return fallback
  }
  async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    const { invoke: tauriInvoke } = await import('@tauri-apps/api/core')
    return tauriInvoke<T>(command, args)
  }
  function applySession(status: SessionStatus) {
    if (status.generation < session.value.generation) return false
    const wasSameSession = status.generation === session.value.generation && status.mode === session.value.mode
    sessionEpoch.value++
    session.value = status
    if (status.locked || !wasSameSession) {
      items.value = []
      recent.value = []
      unsupportedItems.value = 0
    }
    if (!status.locked && status.mode === 'demo' && !items.value.length) items.value = demoItems.map(item => ({ ...item, tags: [...item.tags] }))
    return true
  }
  function applySnapshot(snapshot: VaultSnapshot) {
    if (!applySession(snapshot.session) || snapshot.session.locked) return
    // Only bounded display metadata crosses IPC; no passwords, notes or TOTP seeds.
    items.value = snapshot.items
    unsupportedItems.value = snapshot.unsupportedItems || 0
  }
  async function refreshMetadata() {
    if (!desktop.value || locked.value || demo.value) return
    const epoch = sessionEpoch.value
    const snapshot = await invoke<VaultSnapshot>('list_metadata')
    if (epoch === sessionEpoch.value && snapshot.session.generation >= session.value.generation) applySnapshot(snapshot)
  }
  async function initialize() {
    if (desktop.value) {
      const epoch = sessionEpoch.value
      try {
        const status = await invoke<SessionStatus>('session_status')
        if (epoch === sessionEpoch.value) applySession(status)
        await refreshMetadata()
      } catch (error) { notify(errorMessage(error, 'Desktop session unavailable. Please restart Veyra.')) }
    }
  }
  async function sessionChanged(status: SessionStatus) {
    if (applySession(status) && !status.locked) {
      try { await refreshMetadata() }
      catch (error) { notify(errorMessage(error, 'Could not load vault metadata. Try syncing again.')) }
    }
  }
  async function login(request: LoginRequest): Promise<LoginOutcome> {
    if (!desktop.value) throw new Error('Connect from the Veyra desktop application. The browser preview supports sample data only.')
    const epoch = sessionEpoch.value
    const outcome = await invoke<LoginOutcome>('login_vault', { request })
    if (outcome.status === 'authenticated') {
      // The backend generation prevents an interrupted request from reopening a locked session.
      if (epoch === sessionEpoch.value) { if (applySession(outcome.session)) await refreshMetadata() }
      else if (!locked.value && session.value.generation === outcome.session.generation) await refreshMetadata()
    }
    return outcome
  }
  async function unlock() {
    const epoch = sessionEpoch.value
    try {
      const status = desktop.value ? await invoke<SessionStatus>('unlock_demo') : { locked: false, mode: 'demo' as const, generation: session.value.generation + 1, lastSync: null }
      if (epoch === sessionEpoch.value) applySession(status)
    } catch (error) { notify(errorMessage(error, 'Could not open the demonstration vault.')) }
  }
  async function lock() {
    sessionEpoch.value++
    items.value = []
    recent.value = []
    unsupportedItems.value = 0
    session.value = { ...session.value, locked: true, mode: 'disconnected', lastSync: null }
    feedback.value = ''
    try {
      if (desktop.value) applySession(await invoke<SessionStatus>('lock_vault'))
    } catch { notify('This view is locked. Desktop session could not be reached; please restart Veyra.') }
  }
  async function sync() {
    if (!connected.value || syncing.value) return
    syncing.value = true
    const epoch = sessionEpoch.value
    try {
      const snapshot = await invoke<VaultSnapshot>('sync_vault')
      if (epoch === sessionEpoch.value) { applySnapshot(snapshot); notify('Vault synchronized. Read-only access.') }
    } catch (error) { notify(errorMessage(error, 'Could not synchronize. Existing items remain available.')) }
    finally { syncing.value = false }
  }
  function markUsed(id: string) { recent.value = [id, ...recent.value.filter(value => value !== id)].slice(0, 8) }
  function toggleFavorite(id: string) {
    if (locked.value) return
    if (!demo.value) { notify('This connection is read-only. Edit favorites in Vaultwarden.'); return }
    const item = items.value.find(value => value.id === id)
    if (item) item.favorite = !item.favorite
  }
  async function copy(itemId: string, field: CopyField): Promise<boolean> {
    if (locked.value || !items.value.some(item => item.id === itemId)) return false
    const epoch = sessionEpoch.value
    try {
      if (desktop.value) await invoke('copy_field', { itemId, field })
      else if (demo.value) {
        const sample = field === 'username' ? 'alex@veyra.example' : field === 'totp' ? '123456' : 'Veyra-demo-only!2026'
        await navigator.clipboard.writeText(sample)
      } else return false
      if (locked.value || epoch !== sessionEpoch.value) return false
      markUsed(itemId)
      notify(desktop.value ? `${field === 'totp' ? (demo.value ? 'Sample code' : 'One-time code') : field.charAt(0).toUpperCase() + field.slice(1)} copied · clears after 30s` : 'Sample copied · browser demo cannot securely clear clipboard')
      return true
    } catch (error) { notify(errorMessage(error, 'Could not copy this field.')); return false }
  }
  async function reveal(itemId: string): Promise<string> {
    if (locked.value) return ''
    const generation = session.value.generation
    try {
      if (!desktop.value) return demo.value ? 'Veyra-demo-only!2026' : ''
      const result = await invoke<{ value: string; generation: number }>('reveal_field', { itemId, field: 'password' })
      return !locked.value && generation === session.value.generation && result.generation === generation ? result.value : ''
    } catch (error) { notify(errorMessage(error, 'Could not reveal this field.')); return '' }
  }
  async function openWebsite(item: VaultItem): Promise<boolean> {
    if (locked.value || !item.url) return false
    const epoch = sessionEpoch.value
    try {
      if (desktop.value) await invoke('open_website', { url: item.url })
      else {
        const url = new URL(item.url)
        if (url.protocol !== 'https:') throw new Error('HTTPS required')
        window.open(url.href, '_blank', 'noopener,noreferrer')
      }
      if (locked.value || epoch !== sessionEpoch.value) return false
      markUsed(item.id)
      return true
    } catch (error) { notify(errorMessage(error, 'Could not open this HTTPS address.')); return false }
  }
  async function showQuickAccess() {
    if (desktop.value) {
      try { await invoke('show_quick_access'); return true }
      catch (error) { notify(errorMessage(error, 'Quick Access window unavailable.')) }
    }
    return false
  }
  async function hideQuickAccess() { if (desktop.value) await invoke('hide_quick_access') }
  return { session, locked, demo, connected, items, recent, feedback, syncing, unsupportedItems, desktop, applySession, sessionChanged, initialize, login, unlock, lock, sync, copy, reveal, toggleFavorite, openWebsite, notify, showQuickAccess, hideQuickAccess, errorMessage }
}
