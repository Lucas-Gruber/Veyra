<script setup lang="ts">
import { Search, Plus, Star, Clock3, KeyRound, FileText, CreditCard, ContactRound, FolderClosed, Layers, Trash2, Settings2, LockKeyhole, ChevronDown, Command, CircleHelp, ArrowDownAZ, ArrowUpRight, ShieldCheck, X, Menu, Grid2X2, Check, RefreshCw } from '@lucide/vue'
import { searchItems } from '~/utils/search'
import type { ItemKind, SessionStatus } from '~/types/vault'

const vault = useVault()
const prefs = usePreferences()
const activeView = ref('All items')
const query = ref('')
const selectedId = ref('github')
const settingsOpen = ref(false)
const quickOpen = ref(false)
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
const folderNames = computed(() => vault.demo.value ? ['Personal', 'Work'] : [...new Set(vault.items.value.map(item => item.folder).filter(Boolean))].sort())
const filtered = computed(() => {
  if (vault.locked.value) return []
  let items = vault.items.value
  if (activeView.value === 'Favorites') items = items.filter(item => item.favorite)
  else if (activeView.value === 'Recently used') items = items.filter(item => vault.recent.value.includes(item.id))
  else if (folderNames.value.includes(activeView.value)) items = items.filter(item => item.folder === activeView.value)
  else if (['Trash', 'Collections'].includes(activeView.value)) items = []
  else {
    const type = types.find(type => type.label === activeView.value)
    if (type) items = items.filter(item => item.kind === type.kind)
  }
  const result = searchItems(items, query.value)
  if (!query.value && (sort.value === 'recent' || activeView.value === 'Recently used')) result.sort((a, b) => vault.recent.value.indexOf(a.id) - vault.recent.value.indexOf(b.id))
  return result
})
const selected = computed(() => filtered.value.find(item => item.id === selectedId.value) || filtered.value[0])
const isLight = computed(() => prefs.appearance === 'light' || (prefs.appearance === 'system' && !systemDark.value))
watch(vault.locked, () => { query.value = ''; hasSelected.value = false; sidebarOpen.value = false })
watch(isLight, light => document.documentElement.classList.toggle('light', light))
watch(activeView, () => { query.value = ''; hasSelected.value = false; sidebarOpen.value = false })
function selectItem(id: string) { selectedId.value = id; hasSelected.value = true }
async function openQuick() { if (!(await vault.showQuickAccess())) quickOpen.value = true }
function openDetail(id: string) { activeView.value = 'All items'; selectedId.value = id; hasSelected.value = true; quickOpen.value = false }
function count(label: string) {
  if (label === 'All items') return vault.items.value.length
  const kind = types.find(type => type.label === label)?.kind
  if (kind) return vault.items.value.filter(item => item.kind === kind).length
  if (label === 'Favorites') return vault.items.value.filter(item => item.favorite).length
  if (label === 'Recently used') return vault.recent.value.length
  return undefined
}
function onKeyboard(event: KeyboardEvent) {
  if (event.ctrlKey && event.altKey && event.code === 'Space') { event.preventDefault(); if (!settingsOpen.value) void openQuick(); return }
  if (settingsOpen.value || quickOpen.value || quickStandalone.value) return
  if (event.ctrlKey && event.key === ',') { event.preventDefault(); settingsOpen.value = true; return }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); searchInput.value?.focus(); return }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'l') { event.preventDefault(); void vault.lock(); return }
  if (event.key === 'Escape') { query.value = ''; hasSelected.value = false; return }
  if (event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement || event.target instanceof HTMLTextAreaElement) return
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    const index = filtered.value.findIndex(item => item.id === selected.value?.id)
    const next = filtered.value[(index + (event.key === 'ArrowDown' ? 1 : -1) + filtered.value.length) % filtered.value.length]
    if (next) selectItem(next.id)
  }
  if (!selected.value || vault.locked.value) return
  if (event.ctrlKey && !event.altKey) {
    const key = event.key.toLowerCase()
    const field = ({ u: 'username', p: 'password', o: 'totp' } as const)[key as 'u' | 'p' | 'o']
    if (field) { event.preventDefault(); void vault.copy(selected.value.id, field) }
    if (key === 'b') { event.preventDefault(); void vault.openWebsite(selected.value) }
    if (key === 'd') { event.preventDefault(); vault.toggleFavorite(selected.value.id) }
  }
}
let unlisten: (() => void)[] = []
let media: MediaQueryList | undefined
function onSystemTheme(event: MediaQueryListEvent) { systemDark.value = event.matches }
onMounted(async () => {
  prefs.load()
  quickStandalone.value = new URLSearchParams(window.location.search).get('window') === 'quick'
  media = window.matchMedia('(prefers-color-scheme: dark)')
  systemDark.value = media.matches
  document.documentElement.classList.toggle('light', isLight.value)
  media.addEventListener('change', onSystemTheme)
  window.addEventListener('keydown', onKeyboard)
  if (vault.desktop.value) {
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
onBeforeUnmount(() => { window.removeEventListener('keydown', onKeyboard); media?.removeEventListener('change', onSystemTheme); unlisten.forEach(fn => fn()) })
</script>
<template>
  <UApp>
    <div :class="['veyra-app', { 'theme-light': isLight, 'surface-glass': prefs.surface === 'glass', 'standalone-app': quickStandalone }]">
      <QuickAccess v-if="quickStandalone" standalone @open-item="openDetail"/>
      <template v-else>
        <div class="app-frame">
          <aside :class="['sidebar', { 'mobile-open': sidebarOpen }]" aria-label="Main navigation">
            <div class="brand"><BrandMark/><span>Veyra</span><span class="preview-badge">PREVIEW</span><button class="icon-button mobile-sidebar-close" aria-label="Close navigation" @click="sidebarOpen = false"><X :size="18"/></button></div>
            <button class="account-switcher" @click="vault.notify(vault.connected.value ? 'Connected in memory. Lock the vault to sign out and switch accounts.' : 'Connect to Vaultwarden or explore the sample interface.')"><span class="account-avatar">A</span><span><strong>{{ vault.demo.value ? 'Demo vault' : 'Your vault' }}</strong><small><span class="status-dot"/>{{ vault.connected.value ? 'Read-only connection' : vault.demo.value ? 'Local demonstration' : 'Not connected' }}</small></span><ChevronDown :size="14"/></button>
            <div class="navigation-section"><button v-for="entry in nav" :key="entry.label" :class="['nav-item', { active: activeView === entry.label }]" @click="activeView = entry.label"><component :is="entry.icon" :size="18" :stroke-width="1.7"/><span>{{ entry.label }}</span><span v-if="!vault.locked.value && count(entry.label)" class="nav-count">{{ count(entry.label) }}</span></button></div>
            <div class="navigation-section"><p class="nav-heading">YOUR VAULT</p><button v-for="entry in types" :key="entry.label" :class="['nav-item', { active: activeView === entry.label }]" @click="activeView = entry.label"><component :is="entry.icon" :size="18" :stroke-width="1.7"/><span>{{ entry.label }}</span><span v-if="!vault.locked.value && count(entry.label)" class="nav-count">{{ count(entry.label) }}</span></button></div>
            <div class="navigation-section"><p class="nav-heading">ORGANIZE <button class="icon-button" aria-label="About folders" @click="vault.notify('Folders organize sample metadata. Creating folders needs a connected vault.')"><Plus :size="14"/></button></p><button v-for="folder in folderNames" :key="folder" :class="['nav-item', { active: activeView === folder }]" @click="activeView = folder"><FolderClosed :size="17" :stroke-width="1.7"/><span>{{ folder }}</span></button><button :class="['nav-item', { active: activeView === 'Collections' }]" @click="activeView = 'Collections'"><Layers :size="17" :stroke-width="1.7"/><span>Collections</span></button></div>
            <div class="sidebar-bottom"><button :class="['nav-item', { active: activeView === 'Trash' }]" @click="activeView = 'Trash'"><Trash2 :size="17" :stroke-width="1.7"/><span>Trash</span></button><button class="nav-item" aria-label="Settings" @click="settingsOpen = true"><Settings2 :size="17" :stroke-width="1.7"/><span>Settings</span><kbd>Ctrl ,</kbd></button><div class="sidebar-divider"/><button class="quick-launch" @click="openQuick"><span class="quick-launch-icon"><Command :size="18"/></span><span><strong>Quick Access</strong><small>Your vault, a little closer.</small></span><ArrowUpRight :size="15"/></button><div class="sidebar-status"><span><ShieldCheck :size="13"/>{{ vault.locked.value ? 'Vault is locked' : vault.demo.value ? 'Demo vault · in memory' : 'Connected · read-only' }}</span><button v-if="!vault.locked.value" class="icon-button" aria-label="Lock vault" title="Lock vault · Ctrl L" @click="vault.lock"><LockKeyhole :size="15"/></button></div></div>
          </aside>
          <main :class="['workspace', { 'mobile-detail-open': hasSelected }]">
            <header class="workspace-header"><button class="icon-button mobile-menu" aria-label="Open navigation" @click="sidebarOpen = !sidebarOpen"><Menu :size="20"/></button><div class="breadcrumbs"><span>My vault</span><span>/</span><strong>{{ activeView }}</strong></div><div class="header-actions"><span class="demo-pill"><span/>{{ vault.connected.value ? 'Connected · read-only' : vault.demo.value ? 'Demo mode' : 'Not connected' }}</span><button class="icon-button" aria-label="About this preview" @click="vault.notify('Veyra integration preview · read-only personal vaults. Independent security review is pending; offline unlock and write operations are unavailable.')"><CircleHelp :size="18"/></button><span class="window-divider"/><button class="icon-button" aria-label="Open Quick Access" @click="openQuick"><Command :size="18"/></button></div></header>
            <template v-if="vault.locked.value"><LockedVault/></template>
            <template v-else>
              <section class="workspace-title"><div><p class="eyebrow">A PLACE FOR EVERYTHING</p><h1>{{ activeView }}<span>{{ filtered.length }}</span></h1><p>{{ activeView === 'All items' ? 'The keys to your digital life, thoughtfully organized.' : activeView === 'Favorites' ? 'Your everyday essentials. Always close by.' : activeView === 'Recently used' ? 'Pick up right where you left off.' : 'A little less searching. A little more living.' }}</p></div><div class="title-buttons"><button v-if="vault.connected.value" class="secondary-button sync-button" :disabled="vault.syncing.value" @click="vault.sync"><RefreshCw :size="15" :class="{ spin: vault.syncing.value }"/>{{ vault.syncing.value ? 'Syncing…' : 'Sync vault' }}</button><UButton class="primary-button add-item" @click="vault.notify('This version provides read-only access. Create or edit entries in Vaultwarden.')"><Plus :size="16"/>New item</UButton></div></section>
              <p v-if="vault.unsupportedItems.value" class="compatibility-notice" role="status">{{ vault.unsupportedItems.value }} organization or unsupported items are not shown in this read-only version. View them in Vaultwarden.</p>
              <div class="vault-columns">
                <section class="items-pane" aria-label="Vault items">
                  <div class="list-controls"><div class="search-field"><Search :size="16"/><input ref="searchInput" v-model="query" placeholder="Search your vault…" aria-label="Search your vault" autocomplete="off" spellcheck="false"><kbd v-if="!query">Ctrl K</kbd><button v-else class="icon-button" aria-label="Clear search" @click="query = ''"><X :size="14"/></button></div><button class="icon-button sort-button" :title="sort === 'name' ? 'Sort by recently used' : 'Sort by name'" aria-label="Toggle sort order" @click="sort = sort === 'name' ? 'recent' : 'name'"><ArrowDownAZ :size="18"/></button></div>
                  <div class="list-group-label"><span>{{ query ? 'SEARCH RESULTS' : activeView === 'All items' ? 'ALL LOGINS' : activeView.toUpperCase() }}</span><span>{{ sort === 'name' ? 'A – Z' : 'RECENT' }}</span></div>
                  <div class="item-list"><button v-for="item in filtered" :key="item.id" :class="['item-row', { selected: selected?.id === item.id }]" :aria-pressed="selected?.id === item.id" @click="selectItem(item.id)"><ItemIcon :item="item"/><span class="item-row-text"><strong>{{ item.name }}</strong><small>{{ item.username }}</small></span><Star v-if="item.favorite" :size="13" class="item-favorite" fill="currentColor"/><span v-if="selected?.id === item.id" class="selected-indicator"/></button>
                    <div v-if="!filtered.length" class="list-empty"><Search :size="27" :stroke-width="1.5"/><h3>{{ query ? 'Nothing here just yet.' : 'A little breathing room.' }}</h3><p>{{ query ? 'Try a different name or domain.' : activeView === 'Recently used' ? 'Copy a field or open a website to see it here.' : activeView === 'Collections' ? 'Organization collections are not supported in this read-only version.' : vault.demo.value ? 'This sample vault has no items in this view.' : 'No items in this view. Try All items or synchronize your vault.' }}</p></div>
                  </div>
                  <div class="list-footer"><span>{{ filtered.length }} {{ filtered.length === 1 ? 'item' : 'items' }}</span><span>{{ vault.demo.value ? 'Sample data' : 'Read-only vault' }} <span class="tiny-status-dot"/></span></div>
                </section>
                <section class="detail-pane" aria-label="Selected item"><button class="mobile-back secondary-button" @click="hasSelected = false">← Back to items</button><ItemDetail v-if="selected" :key="selected.id" :item="selected"/><div v-else class="detail-empty"><div class="empty-vault-symbol"><BrandMark/></div><h2>Everything in its place.</h2><p>Select an item to see the details.</p><span>Your vault. One shortcut away.</span></div></section>
              </div>
            </template>
          </main>
        </div>
        <footer class="app-statusbar"><span><span class="status-dot"/>Integration preview<span class="statusbar-divider">·</span>{{ vault.connected.value ? 'Connected in memory' : 'No server connected' }}</span><span><LockKeyhole :size="12"/>{{ vault.locked.value ? 'Locked · signed out' : vault.demo.value ? 'Synthetic data only' : 'Read-only · no offline cache' }}<span class="statusbar-divider">·</span><button @click="openQuick">Quick Access <kbd>Ctrl Alt Space</kbd></button></span></footer>
        <QuickAccess v-if="quickOpen" @close="quickOpen = false" @open-item="openDetail"/>
        <SettingsDialog v-if="settingsOpen" @close="settingsOpen = false"/>
      </template>
      <Transition name="toast"><div v-if="vault.feedback.value" class="feedback-toast" role="status" aria-live="polite"><Check :size="16"/>{{ vault.feedback.value }}</div></Transition>
    </div>
  </UApp>
</template>
