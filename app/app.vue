<script setup lang="ts">
import { Search, Plus, Star, Clock3, KeyRound, FileText, CreditCard, ContactRound, Layers, Trash2, Settings2, LockKeyhole, Command, CircleHelp, ArrowDownAZ, ArrowUpRight, ShieldCheck, X, Menu, Grid2X2, Check, RefreshCw } from '@lucide/vue'
import { searchItems } from '~/utils/search'
import { folderContainsItem } from '~/utils/folders'
import { matchesShortcut } from '~/utils/shortcuts'
import type { ItemKind, SessionStatus, VaultItem } from '~/types/vault'
import type { ContextAction } from '~/components/ItemContextMenu.vue'
import type { ManagementMode } from '~/components/ManageItemsDialog.vue'

const vault = useVault()
const prefs = usePreferences()
const appUpdates = useAppUpdates()
const activeView = ref('All items')
const vaultScope = ref('all')
const activeCollection = ref('')
const activeFolder = ref<string | null>(null)
const scopeItems = computed(() => vault.items.value.filter(item => vaultScope.value === 'all' || (vaultScope.value === 'personal' ? !item.organizationId : item.organizationId === vaultScope.value)))
const scopedItems = computed(() => scopeItems.value.filter(item => !item.deleted))
const scopedCollections = computed(() => vault.collections.value.filter(collection => vaultScope.value === 'all' || collection.organizationId === vaultScope.value))
const vaultName = computed(() => vaultScope.value === 'all' ? 'All vaults' : vaultScope.value === 'personal' ? 'Your vault' : vault.organizations.value.find(org => org.id === vaultScope.value)?.name || 'All vaults')
watch(vaultScope, () => { activeView.value = 'All items'; activeFolder.value = null; query.value = ''; hasSelected.value = false; activeCollection.value = '' })
const query = ref('')
const selectedId = ref('github')
const selectedIds = ref<string[]>([])
const selectionAnchor = ref<string>()
const contextMenu = ref<{ x: number; y: number }>()
const management = ref<{ mode: ManagementMode; items: VaultItem[] }>()
const settingsOpen = ref(false)
const quickOpen = ref(false)
const editorOpen = ref(false)
const editingItem = ref<VaultItem>()
const sidebarOpen = ref(false)
const sort = ref<'name' | 'recent'>('name')
const searchInput = ref<HTMLInputElement>()
const quickStandalone = ref(false)
const systemDark = ref(true)
const hasSelected = ref(false)
const nav = [
  { label: 'All items', icon: Grid2X2 }, { label: 'Favorites', icon: Star }, { label: 'Recently used', icon: Clock3 },
]
const types: { label: string; icon: typeof KeyRound; kind: ItemKind }[] = [
  { label: 'Logins', icon: KeyRound, kind: 'login' }, { label: 'Secure notes', icon: FileText, kind: 'note' }, { label: 'Cards', icon: CreditCard, kind: 'card' }, { label: 'Identities', icon: ContactRound, kind: 'identity' },
]
const viewTitle = computed(() => activeFolder.value ?? activeView.value)
const filtered = computed(() => {
  if (vault.locked.value) return []
  let items = scopedItems.value
  if (activeFolder.value !== null) items = items.filter(item => folderContainsItem(item, activeFolder.value!, vault.folders.value))
  else if (activeView.value === 'Favorites') items = items.filter(item => item.favorite)
  else if (activeView.value === 'Recently used') items = items.filter(item => vault.recent.value.includes(item.id))
  else if (activeView.value === 'Trash') items = scopeItems.value.filter(item => item.deleted)
  else if (activeView.value === 'Collections') items = items.filter(item => activeCollection.value ? item.collectionIds?.includes(activeCollection.value) : item.collectionIds?.length)
  else {
    const type = types.find(type => type.label === activeView.value)
    if (type) items = items.filter(item => item.kind === type.kind)
  }
  const result = searchItems(items, query.value)
  if (!query.value && (sort.value === 'recent' || activeView.value === 'Recently used')) result.sort((a, b) => vault.recent.value.indexOf(a.id) - vault.recent.value.indexOf(b.id))
  return result
})
const selected = computed(() => filtered.value.find(item => item.id === selectedId.value) || filtered.value[0])
const selectedItems = computed(() => filtered.value.filter(item => selectedIds.value.includes(item.id)))
watch(() => vault.session.value.generation, () => { selectedIds.value = []; selectionAnchor.value = undefined; contextMenu.value = undefined; management.value = undefined })
watch(() => filtered.value.map(item => item.id).join(','), () => {
  const visible = new Set(filtered.value.map(item => item.id))
  selectedIds.value = selectedIds.value.filter(id => visible.has(id))
  if (selectionAnchor.value && !visible.has(selectionAnchor.value)) selectionAnchor.value = undefined
  contextMenu.value = undefined
})
const contextActions = computed<ContextAction[]>(() => {
  const items = selectedItems.value
  const one = items.length === 1 ? items[0] : undefined
  const busy = vault.mutating.value || vault.needsSync.value
  const editable = vault.connected.value && !busy && items.length > 0 && items.every(item => !item.readOnly && !item.deleted)
  return [
    ...one && !one.deleted ? [
      { id: 'detail', label: 'Show item' },
      { id: 'website', label: 'Open website', disabled: one.kind !== 'login' || !one.url },
      { id: 'username', label: 'Copy username', disabled: one.kind !== 'login' },
      { id: 'password', label: 'Copy password', disabled: one.kind !== 'login' || one.hasPassword === false },
      { id: 'totp', label: 'Copy one-time code', disabled: one.kind !== 'login' || one.hasTotp === false },
      { id: 'edit', label: 'Edit item', disabled: !editable },
      { id: 'favorite', label: one.favorite ? 'Remove from favorites' : 'Add to favorites', disabled: !(vault.demo.value || editable) },
    ] : [],
    { id: 'folder', label: 'Assign folder…', disabled: !editable },
    { id: 'collections', label: 'Assign collections…', disabled: !editable || !items[0]?.organizationId || items.some(item => item.organizationId !== items[0]?.organizationId) },
    { id: 'copy', label: 'Duplicate…', disabled: !editable },
    { id: 'move', label: 'Move to another vault…', disabled: !editable },
    { id: 'trash', label: 'Move to trash…', danger: true, disabled: !vault.connected.value || busy || !items.length || items.some(item => !item.canDelete || item.deleted) },
    { id: 'restore', label: 'Restore items…', disabled: !vault.connected.value || busy || !items.length || items.some(item => !item.canRestore || !item.deleted) },
  ]
})
const isLight = computed(() => prefs.appearance === 'light' || (prefs.appearance === 'system' && !systemDark.value))
watch(vault.locked, locked => { query.value = ''; hasSelected.value = false; sidebarOpen.value = false; if (locked) { activeFolder.value = null; selectedIds.value = []; contextMenu.value = undefined; management.value = undefined; editorOpen.value = false; editingItem.value = undefined } })
watch(isLight, light => document.documentElement.classList.toggle('light', light))
watch(() => prefs.textSize, size => { document.documentElement.style.fontSize = `${size}px` })
watch(activeView, () => { activeFolder.value = null; query.value = ''; hasSelected.value = false; sidebarOpen.value = false })
function selectFolder(path: string) { activeFolder.value = path; query.value = ''; hasSelected.value = false; sidebarOpen.value = false }
function selectView(view: string) { activeFolder.value = null; activeView.value = view }
function selectItem(id: string, event?: MouseEvent) {
  const range = event?.shiftKey && selectionAnchor.value
  if (range) {
    const start = filtered.value.findIndex(item => item.id === selectionAnchor.value)
    const end = filtered.value.findIndex(item => item.id === id)
    if (start >= 0 && end >= 0) {
      const ids = filtered.value.slice(Math.min(start, end), Math.max(start, end) + 1).map(item => item.id)
      selectedIds.value = event?.ctrlKey || event?.metaKey ? [...new Set([...selectedIds.value, ...ids])] : ids
    }
  } else if (event?.ctrlKey || event?.metaKey) {
    selectedIds.value = selectedIds.value.includes(id) ? selectedIds.value.filter(value => value !== id) : [...selectedIds.value, id]
    selectionAnchor.value = id
  } else { selectedIds.value = [id]; selectionAnchor.value = id }
  selectedId.value = id; hasSelected.value = !event?.shiftKey && !event?.ctrlKey && !event?.metaKey
}
function openContext(item: VaultItem, event: MouseEvent | KeyboardEvent) {
  event.preventDefault(); event.stopPropagation()
  if (!selectedIds.value.includes(item.id)) selectItem(item.id)
  const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect()
  contextMenu.value = { x: event instanceof MouseEvent && event.clientX ? event.clientX : bounds.left + 20, y: event instanceof MouseEvent && event.clientY ? event.clientY : bounds.top + 20 }
}
function openManagement(mode: ManagementMode) {
  if (!selectedItems.value.length) return
  management.value = { mode, items: selectedItems.value.map(item => ({ ...item, collectionIds: [...(item.collectionIds || [])] })) }
  contextMenu.value = undefined
}
function contextAction(action: string) {
  if (contextActions.value.find(entry => entry.id === action)?.disabled) return
  const item = selectedItems.value[0]
  contextMenu.value = undefined
  if (['folder', 'collections', 'copy', 'move', 'trash', 'restore'].includes(action)) { openManagement(action as ManagementMode); return }
  if (!item) return
  if (action === 'detail') { selectItem(item.id); hasSelected.value = true }
  else if (action === 'edit') openEditor(item)
  else if (action === 'website') void vault.openWebsite(item)
  else if (action === 'favorite') void vault.toggleFavorite(item.id)
  else if (['username', 'password', 'totp'].includes(action)) void vault.copy(item.id, action as 'username' | 'password' | 'totp')
}
async function openQuick() { if (!(await vault.showQuickAccess())) quickOpen.value = true }
async function openDetail(id: string) { vaultScope.value = 'all'; selectView('All items'); quickOpen.value = false; await nextTick(); selectItem(id) }
function openEditor(item?: VaultItem) { editingItem.value = item; editorOpen.value = true }
function count(label: string) {
  if (label === 'All items') return scopedItems.value.length
  const kind = types.find(type => type.label === label)?.kind
  if (kind) return scopedItems.value.filter(item => item.kind === kind).length
  if (label === 'Favorites') return scopedItems.value.filter(item => item.favorite).length
  if (label === 'Recently used') return scopedItems.value.filter(item => vault.recent.value.includes(item.id)).length
  return undefined
}
function onKeyboard(event: KeyboardEvent) {
  if (event.defaultPrevented) return
  const matches = (action: Parameters<typeof matchesShortcut>[2]) => matchesShortcut(event, prefs.shortcuts, action)
  if (matches('lock')) { event.preventDefault(); void vault.lock(); return }
  if (editorOpen.value || management.value || contextMenu.value) return
  if (matches('openQuick')) { event.preventDefault(); if (!settingsOpen.value) void openQuick(); return }
  if (settingsOpen.value || quickOpen.value || quickStandalone.value) return
  if (matches('settings')) { event.preventDefault(); settingsOpen.value = true; return }
  if (matches('search')) { event.preventDefault(); searchInput.value?.focus(); return }
  if (event.key === 'Escape') { query.value = ''; hasSelected.value = false; return }
  if (event.target instanceof HTMLElement && (event.target.isContentEditable || event.target.closest('input, select, textarea'))) return
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'a') { event.preventDefault(); selectedIds.value = filtered.value.map(item => item.id); return }
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    const index = filtered.value.findIndex(item => item.id === selected.value?.id)
    const next = filtered.value[(index + (event.key === 'ArrowDown' ? 1 : -1) + filtered.value.length) % filtered.value.length]
    if (next) selectItem(next.id)
    return
  }
  if (!selected.value || vault.locked.value || selected.value.deleted) return
  const field = (['username', 'password', 'totp'] as const).find(matches)
  if (field) { event.preventDefault(); void vault.copy(selected.value.id, field) }
  else if (matches('website')) { event.preventDefault(); void vault.openWebsite(selected.value) }
  else if (matches('favorite')) { event.preventDefault(); void vault.toggleFavorite(selected.value.id) }
}
let unlisten: (() => void)[] = []
let media: MediaQueryList | undefined
let stopAutomaticUpdateChecks: (() => void) | undefined
function onSystemTheme(event: MediaQueryListEvent) { systemDark.value = event.matches }
function onPreferencesChanged(event: StorageEvent) { if (event.key === 'veyra:preferences' || event.key === null) prefs.load() }
onMounted(async () => {
  prefs.load()
  document.documentElement.style.fontSize = `${prefs.textSize}px`
  stopAutomaticUpdateChecks = appUpdates.startAutomaticChecks()
  quickStandalone.value = new URLSearchParams(window.location.search).get('window') === 'quick'
  media = window.matchMedia('(prefers-color-scheme: dark)')
  systemDark.value = media.matches
  document.documentElement.classList.toggle('light', isLight.value)
  media.addEventListener('change', onSystemTheme)
  window.addEventListener('keydown', onKeyboard)
  window.addEventListener('storage', onPreferencesChanged)
  if (vault.desktop.value) {
    if (!quickStandalone.value) {
      const { invoke } = await import('@tauri-apps/api/core')
      void invoke('configure_quick_access_shortcut', { shortcut: prefs.shortcuts.openQuick }).catch(() => undefined)
    }
    const { listen } = await import('@tauri-apps/api/event')
    unlisten = await Promise.all([
      listen<SessionStatus>('vault:locked', event => void vault.sessionChanged(event.payload)),
      listen<SessionStatus>('vault:unlocked', event => void vault.sessionChanged(event.payload)),
      listen<SessionStatus>('vault:changed', event => void vault.sessionChanged(event.payload)),
      listen<string>('vault:open-item', event => openDetail(event.payload)),
    ])
  }
  // Subscribe before requesting the snapshot; useVault also guards against newer events.
  await vault.initialize()
})
onBeforeUnmount(() => { stopAutomaticUpdateChecks?.(); window.removeEventListener('keydown', onKeyboard); window.removeEventListener('storage', onPreferencesChanged); media?.removeEventListener('change', onSystemTheme); unlisten.forEach(fn => fn()) })
</script>
<template>
  <UApp>
    <div :class="['veyra-app', { 'theme-light': isLight, 'surface-glass': prefs.surface === 'glass', 'standalone-app': quickStandalone }]">
      <QuickAccess v-if="quickStandalone" standalone @open-item="openDetail"/>
      <template v-else>
        <div class="app-frame">
          <aside :class="['sidebar', { 'mobile-open': sidebarOpen }]" aria-label="Main navigation">
            <div class="brand"><BrandMark/><span>Veyra</span><span class="preview-badge">PREVIEW</span><button class="icon-button mobile-sidebar-close" aria-label="Close navigation" @click="sidebarOpen = false"><X :size="18"/></button></div>
            <VaultSwitcher v-model="vaultScope"/>
            <div class="navigation-section"><button v-for="entry in nav" :key="entry.label" :class="['nav-item', { active: activeFolder === null && activeView === entry.label }]" @click="selectView(entry.label)"><component :is="entry.icon" :size="18" :stroke-width="1.7"/><span>{{ entry.label }}</span><span v-if="!vault.locked.value && count(entry.label)" class="nav-count">{{ count(entry.label) }}</span></button></div>
            <div class="navigation-section"><p class="nav-heading">YOUR VAULT</p><button v-for="entry in types" :key="entry.label" :class="['nav-item', { active: activeFolder === null && activeView === entry.label }]" @click="selectView(entry.label)"><component :is="entry.icon" :size="18" :stroke-width="1.7"/><span>{{ entry.label }}</span><span v-if="!vault.locked.value && count(entry.label)" class="nav-count">{{ count(entry.label) }}</span></button></div>
            <div class="navigation-section"><p class="nav-heading">ORGANIZE <button class="icon-button" aria-label="About folders" @click="vault.notify('Folders are synchronized from Vaultwarden. Select items and choose Assign folder from their context menu.')"><Plus :size="14"/></button></p><FolderTree v-if="!vault.locked.value" :folders="vault.folders.value" :items="scopedItems" :selected-path="activeFolder" @select="selectFolder"/><button :class="['nav-item', { active: activeFolder === null && activeView === 'Collections' }]" @click="selectView('Collections')"><Layers :size="17" :stroke-width="1.7"/><span>Collections</span></button></div>
            <div class="sidebar-bottom"><button :class="['nav-item', { active: activeFolder === null && activeView === 'Trash' }]" @click="selectView('Trash')"><Trash2 :size="17" :stroke-width="1.7"/><span>Trash</span></button><button class="nav-item" aria-label="Settings" @click="settingsOpen = true"><Settings2 :size="17" :stroke-width="1.7"/><span>Settings</span><kbd>{{ prefs.shortcuts.settings }}</kbd></button><div class="sidebar-divider"/><button class="quick-launch" @click="openQuick"><span class="quick-launch-icon"><Command :size="18"/></span><span><strong>Quick Access</strong><small>Your vault, a little closer.</small></span><ArrowUpRight :size="15"/></button><div class="sidebar-status"><span><ShieldCheck :size="13"/>{{ vault.locked.value ? 'Vault is locked' : vault.demo.value ? 'Demo vault · in memory' : 'Connected' }}</span><button v-if="!vault.locked.value" class="icon-button" aria-label="Lock vault" :title="`Lock vault · ${prefs.shortcuts.lock}`" @click="vault.lock"><LockKeyhole :size="15"/></button></div></div>
          </aside>
          <main :class="['workspace', { 'mobile-detail-open': hasSelected }]">
            <header class="workspace-header"><button class="icon-button mobile-menu" aria-label="Open navigation" @click="sidebarOpen = !sidebarOpen"><Menu :size="20"/></button><div class="breadcrumbs"><span>{{ vaultName }}</span><span>/</span><strong>{{ viewTitle }}</strong></div><div class="header-actions"><span class="demo-pill"><span/>{{ vault.connected.value ? 'Connected' : vault.demo.value ? 'Demo mode' : 'Not connected' }}</span><button class="icon-button" aria-label="About this preview" @click="vault.notify('Veyra integration preview · personal and organization vaults. Independent security review is pending; offline unlock is unavailable.')"><CircleHelp :size="18"/></button><span class="window-divider"/><button class="icon-button" aria-label="Open Quick Access" @click="openQuick"><Command :size="18"/></button></div></header>
            <template v-if="vault.locked.value"><LockedVault/></template>
            <template v-else>
              <section class="workspace-title"><div><p class="eyebrow">A PLACE FOR EVERYTHING</p><h1>{{ viewTitle }}<span>{{ filtered.length }}</span></h1><p>{{ activeView === 'All items' ? 'The keys to your digital life, thoughtfully organized.' : activeView === 'Favorites' ? 'Your everyday essentials. Always close by.' : activeView === 'Recently used' ? 'Pick up right where you left off.' : 'A little less searching. A little more living.' }}</p></div><div class="title-buttons"><button v-if="vault.connected.value" class="secondary-button sync-button" :disabled="vault.syncing.value" @click="vault.sync"><RefreshCw :size="15" :class="{ spin: vault.syncing.value }"/>{{ vault.syncing.value ? 'Syncing…' : 'Sync vault' }}</button><UButton class="primary-button add-item" :disabled="!vault.connected.value || vault.needsSync.value" @click="openEditor()"><Plus :size="16"/>New item</UButton></div></section>
              <p v-if="vault.needsSync.value" class="compatibility-notice" role="alert">A write needs verification. Synchronize and inspect the vault before making another change.</p>
              <p v-if="vault.unsupportedItems.value" class="compatibility-notice" role="status">{{ vault.unsupportedItems.value }} items use unsupported formats and are not shown. View them in Vaultwarden.</p>
              <div v-if="activeFolder === null && activeView === 'Collections'" class="collection-filters" aria-label="Collections"><button :aria-pressed="!activeCollection" @click="activeCollection = ''">All collections</button><button v-for="collection in scopedCollections" :key="collection.id" :aria-pressed="activeCollection === collection.id" @click="activeCollection = collection.id">{{ collection.name }}</button></div>
              <div class="vault-columns">
                <section class="items-pane" aria-label="Vault items">
                  <div class="list-controls"><div class="search-field"><Search :size="16"/><input ref="searchInput" v-model="query" placeholder="Search your vault…" aria-label="Search your vault" autocomplete="off" spellcheck="false"><kbd v-if="!query">{{ prefs.shortcuts.search }}</kbd><button v-else class="icon-button" aria-label="Clear search" @click="query = ''"><X :size="14"/></button></div><button class="icon-button sort-button" :title="sort === 'name' ? 'Sort by recently used' : 'Sort by name'" aria-label="Toggle sort order" @click="sort = sort === 'name' ? 'recent' : 'name'"><ArrowDownAZ :size="18"/></button></div>
                  <div class="list-group-label"><span>{{ query ? 'SEARCH RESULTS' : activeFolder === null && activeView === 'All items' ? 'ALL ITEMS' : viewTitle.toUpperCase() }}</span><span>{{ sort === 'name' ? 'A – Z' : 'RECENT' }}</span></div>
                  <div v-if="selectedIds.length > 1" class="selection-toolbar" role="toolbar" aria-label="Selected items"><span>{{ selectedIds.length }} selected</span><button class="secondary-button" @click="openManagement('folder')">Assign folder</button><button class="secondary-button" @click="contextMenu = { x: $event.clientX, y: $event.clientY }">Actions</button><button class="icon-button" aria-label="Clear selection" @click="selectedIds = []"><X :size="15"/></button></div>
                  <div class="item-list"><button v-for="item in filtered" :key="item.id" :class="['item-row', { selected: selectedIds.includes(item.id) || (!selectedIds.length && selected?.id === item.id) }]" :aria-pressed="selectedIds.includes(item.id)" @click="selectItem(item.id, $event)" @contextmenu="openContext(item, $event)" @keydown.shift.f10="openContext(item, $event)"><ItemIcon :item="item"/><span class="item-row-text"><strong>{{ item.name }}</strong><small>{{ item.username }}</small></span><Star v-if="item.favorite" :size="13" class="item-favorite" fill="currentColor"/><span v-if="selected?.id === item.id" class="selected-indicator"/></button>
                    <div v-if="!filtered.length" class="list-empty"><Search :size="27" :stroke-width="1.5"/><h3>{{ query ? 'Nothing here just yet.' : 'A little breathing room.' }}</h3><p>{{ query ? 'Try a different name or domain.' : activeView === 'Recently used' ? 'Copy a field or open a website to see it here.' : activeView === 'Collections' ? 'No accessible items in this collection.' : vault.demo.value ? 'This sample vault has no items in this view.' : 'No items in this view. Try All items or synchronize your vault.' }}</p></div>
                  </div>
                  <div class="list-footer"><span>{{ filtered.length }} {{ filtered.length === 1 ? 'item' : 'items' }}</span><span>{{ vault.demo.value ? 'Sample data' : 'Vaultwarden' }} <span class="tiny-status-dot"/></span></div>
                </section>
                <section class="detail-pane" aria-label="Selected item"><button class="mobile-back secondary-button" @click="hasSelected = false">← Back to items</button><ItemDetail v-if="selected" :key="selected.id" :item="selected" @edit="openEditor(selected)"/><div v-else class="detail-empty"><div class="empty-vault-symbol"><BrandMark/></div><h2>Everything in its place.</h2><p>Select an item to see the details.</p><span>Your vault. One shortcut away.</span></div></section>
              </div>
            </template>
          </main>
        </div>
        <footer class="app-statusbar"><span><span class="status-dot"/>Integration preview<span class="statusbar-divider">·</span>{{ vault.connected.value ? 'Connected in memory' : 'No server connected' }}</span><span><LockKeyhole :size="12"/>{{ vault.locked.value ? 'Locked · signed out' : vault.demo.value ? 'Synthetic data only' : 'No offline cache' }}<span class="statusbar-divider">·</span><button @click="openQuick">Quick Access <kbd>{{ prefs.shortcuts.openQuick }}</kbd></button></span></footer>
        <QuickAccess v-if="quickOpen" @close="quickOpen = false" @open-item="openDetail"/>
        <ItemContextMenu v-if="contextMenu" :x="contextMenu.x" :y="contextMenu.y" :actions="contextActions" @close="contextMenu = undefined" @select="contextAction"/>
        <ManageItemsDialog v-if="management" :items="management.items" :mode="management.mode" @close="management = undefined"/>
        <SettingsDialog v-if="settingsOpen" @close="settingsOpen = false"/>
        <ItemEditor v-if="editorOpen" :item="editingItem" :organization-id="['all', 'personal'].includes(vaultScope) ? null : vaultScope" @close="editorOpen = false; editingItem = undefined" @saved="openDetail"/>
      </template>
      <Transition name="toast"><div v-if="vault.feedback.value" class="feedback-toast" role="status" aria-live="polite"><Check :size="16"/>{{ vault.feedback.value }}</div></Transition>
    </div>
  </UApp>
</template>

<style scoped>
.selection-toolbar { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; padding: 8px 0; font-size: .8125rem; color: var(--muted); }
.selection-toolbar .secondary-button { padding: 6px 8px; font-size: .75rem; }
</style>
