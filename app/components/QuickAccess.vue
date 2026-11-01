<script setup lang="ts">
import { Search, ArrowUpRight, KeyRound, Star, CornerDownLeft, ChevronRight, X, ExternalLink, UserRound, Clock3 } from '@lucide/vue'
import type { VaultItem } from '~/types/vault'
import { highlightSegments, searchItems } from '~/utils/search'
import { hasSelectedInputText, matchesShortcut, type ShortcutAction } from '~/utils/shortcuts'
import { clearQuickAccessSearch, rememberQuickAccessSearch, restoreQuickAccessSearch } from '~/utils/quickAccessSearch'
const props = defineProps<{ standalone?: boolean }>()
const emit = defineEmits<{ close: []; openItem: [id: string] }>()
const vault = useVault()
const prefs = usePreferences()
const restored = vault.locked.value ? undefined : restoreQuickAccessSearch(vault.session.value.generation)
const query = ref(restored?.query ?? '')
const selectedItemId = ref<string | null>(restored?.selectedItemId ?? null)
let dismissed = false
const actions = ref(false)
const searchInput = ref<HTMLInputElement>()
const dialog = ref<HTMLElement>()
const loginForm = ref<{ clear: () => void }>()
let previousFocus: HTMLElement | null = null
let unlistenFocus: (() => void) | undefined
let unlistenBlur: (() => void) | undefined
const results = computed(() => vault.locked.value ? [] : searchItems(vault.items.value.filter(item => !item.deleted), query.value).slice(0, 8))
const selected = computed({
  get: () => Math.max(0, results.value.findIndex(item => item.id === selectedItemId.value)),
  set: (index: number) => { selectedItemId.value = results.value[index]?.id ?? null },
})
const current = computed(() => results.value[selected.value])
watch(query, () => { selectedItemId.value = null; actions.value = false }, { flush: 'sync' })
watch([vault.locked, () => vault.session.value.generation], ([locked]) => {
  clearQuickAccessSearch()
  reset()
  if (!locked) nextTick(() => searchInput.value?.focus())
}, { flush: 'sync' })
watch(selected, () => nextTick(() => document.querySelector('.quick-result.selected')?.scrollIntoView({ block: 'nearest' })))
onMounted(async () => {
  previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
  await nextTick()
  if (vault.locked.value) { const target = dialog.value?.querySelector<HTMLElement>('input, .demo-entry button'); target?.focus() }
  else searchInput.value?.focus()
  if (vault.desktop.value) {
    const { listen } = await import('@tauri-apps/api/event')
    unlistenFocus = await listen('quick-access:focus', () => {
      prefs.load()
      if (dismissed) {
        const saved = vault.locked.value ? undefined : restoreQuickAccessSearch(vault.session.value.generation)
        query.value = saved?.query ?? ''
        selectedItemId.value = saved?.selectedItemId ?? null
        dismissed = false
      }
      nextTick(() => { if (vault.locked.value) dialog.value?.querySelector<HTMLInputElement>('input')?.focus(); else searchInput.value?.focus() })
    })
    unlistenBlur = await listen('quick-access:blur', dismiss)
  }
})
onBeforeUnmount(() => { dismiss(); unlistenFocus?.(); unlistenBlur?.(); previousFocus?.focus() })
function reset() { query.value = ''; actions.value = false; selectedItemId.value = null; loginForm.value?.clear() }
function dismiss() {
  if (!dismissed) {
    if (vault.locked.value) clearQuickAccessSearch()
    else rememberQuickAccessSearch(query.value, vault.session.value.generation, current.value?.id ?? null)
    dismissed = true
  }
  reset()
}
async function close() {
  dismiss()
  if (props.standalone) await vault.hideQuickAccess()
  else emit('close')
}
async function run(action: string, item: VaultItem | undefined = current.value) {
  if (!item) return
  if (item.kind !== 'login' && action === 'autofill') action = 'detail'
  if (item.kind !== 'login' && ['username', 'password', 'totp', 'website'].includes(action)) { vault.notify('Credential actions are available for login items. Open the item to view its contents.'); return }
  if (action === 'favorite') { vault.toggleFavorite(item.id); return }
  if (action === 'detail') {
    try {
      if (props.standalone && vault.desktop.value) {
        const { invoke } = await import('@tauri-apps/api/core')
        await invoke('open_main_item', { itemId: item.id })
      } else { emit('openItem', item.id); await close() }
    } catch { vault.notify('Could not open this item in the main window.') }
    return
  }
  let succeeded: boolean
  if (action === 'autofill') succeeded = await vault.openAndFill(item)
  else if (action === 'website') succeeded = await vault.openWebsite(item)
  else succeeded = await vault.copy(item.id, action as 'username' | 'password' | 'totp')
  if (!succeeded) return
  actions.value = false
  try { await close() }
  catch { vault.notify('Action completed. Could not close Quick Access.') }
}
async function onKey(event: KeyboardEvent) {
  const matches = (action: ShortcutAction) => matchesShortcut(event, prefs.shortcuts, action)
  if (matches('quickClose') || event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); if (actions.value) { actions.value = false; searchInput.value?.focus() } else await close(); return }
  if (vault.locked.value) {
    if (event.key === 'Tab') trapTab(event)
    return
  }
  if (matches('quickNext') || matches('quickPrevious')) {
    event.preventDefault(); event.stopPropagation()
    const direction = matches('quickNext') ? 1 : -1
    if (actions.value) {
      const buttons = Array.from(dialog.value?.querySelectorAll<HTMLButtonElement>('.quick-actions button') || [])
      const index = buttons.indexOf(document.activeElement as HTMLButtonElement)
      buttons[(index + direction + buttons.length) % buttons.length]?.focus()
    } else selected.value = (selected.value + direction + results.value.length) % Math.max(1, results.value.length)
    return
  }
  if (matches('quickActions') || (event.key === 'Tab' && event.shiftKey && prefs.shortcuts.quickActions === 'Tab')) {
    if (!actions.value) {
      event.preventDefault(); event.stopPropagation(); actions.value = true
      nextTick(() => {
        const buttons = Array.from(dialog.value?.querySelectorAll<HTMLButtonElement>('.quick-actions button') || [])
        ;(event.shiftKey ? buttons.at(-1) : buttons[0])?.focus()
      })
    } else if (event.key !== 'Tab') { event.preventDefault(); actions.value = false; searchInput.value?.focus() }
    else trapTab(event, true)
    return
  }
  if (event.key === 'Tab') { trapTab(event, actions.value); return }
  if (matches('search')) { event.preventDefault(); searchInput.value?.focus(); return }
  // Enter on an action button keeps the button's standard activation behavior.
  if (actions.value && event.key === 'Enter') return
  if (matches('username') && hasSelectedInputText(event.target)) return
  const action = (['username', 'password', 'totp', 'website', 'autofill', 'detail', 'favorite'] as const).find(matches)
  if (action) { event.preventDefault(); event.stopPropagation(); await run(action) }
}
function trapTab(event: KeyboardEvent, actionMenu = false) {
  const selector = actionMenu ? '.quick-actions button' : 'input:not([disabled]), select:not([disabled]), button:not([disabled])'
  const fields = Array.from(dialog.value?.querySelectorAll<HTMLElement>(selector) || [])
  const first = fields[0], last = fields.at(-1)
  if ((event.shiftKey && document.activeElement === first) || (!event.shiftKey && document.activeElement === last)) {
    event.preventDefault()
    if (actionMenu) { actions.value = false; searchInput.value?.focus() }
    else (event.shiftKey ? last : first)?.focus()
  }
}
const actionList = computed(() => [
  { label: 'Copy password', action: 'password', icon: KeyRound },
  { label: 'Copy username', action: 'username', icon: UserRound },
  { label: 'Copy one-time code', action: 'totp', icon: Clock3 },
  { label: 'Open website', action: 'website', icon: ArrowUpRight },
  { label: 'Open and fill with companion', action: 'autofill', icon: ExternalLink },
  { label: 'Open in Veyra', action: 'detail', icon: ExternalLink },
  { label: 'Toggle favorite', action: 'favorite', icon: Star },
].map(entry => ({ ...entry, key: prefs.shortcuts[entry.action as ShortcutAction] })))
</script>
<template>
  <div :class="standalone ? 'quick-standalone' : 'quick-backdrop'" @click.self="close">
    <section ref="dialog" class="quick-access" role="dialog" aria-modal="true" aria-label="Quick Access" @keydown="onKey">
      <template v-if="vault.locked.value"><button class="icon-button quick-close" aria-label="Close Quick Access" @click="close"><X :size="18"/></button><LockedVault ref="loginForm" compact/></template>
      <template v-else>
        <div class="quick-search"><Search :size="23" :stroke-width="1.7"/><input ref="searchInput" v-model="query" placeholder="Where do you want to go?" aria-label="Search vault" autocomplete="off" spellcheck="false" role="combobox" aria-autocomplete="list" aria-controls="quick-results" :aria-expanded="results.length > 0" :aria-activedescendant="current ? `quick-${current.id}` : undefined"><kbd @click="close">{{ prefs.shortcuts.quickClose }}</kbd></div>
        <div class="quick-context"><span>{{ query ? `${results.length} MATCH${results.length === 1 ? '' : 'ES'}` : 'YOUR EVERYDAY ESSENTIALS' }}</span><span class="demo-dot">{{ vault.demo.value ? 'DEMO VAULT' : 'CONNECTED' }}</span></div>
        <div id="quick-results" class="quick-results" role="listbox" aria-label="Matching items">
          <div v-for="(item, index) in results" :id="`quick-${item.id}`" :key="item.id" :class="['quick-result', { selected: selected === index }]" role="option" :aria-selected="selected === index" tabindex="-1" @mousemove="selected = index" @click="run(prefs.defaultAction, item)">
            <ItemIcon :item="item"/><span class="quick-result-text"><strong><template v-for="(segment, i) in highlightSegments(item.name, query)" :key="i"><mark v-if="segment.match">{{ segment.text }}</mark><template v-else>{{ segment.text }}</template></template></strong><small>{{ item.username }}</small></span><span class="quick-domain">{{ item.domain }}</span><button v-if="item.kind === 'login' && item.url" class="quick-open-button" :aria-label="`Open website: ${item.name}`" title="Open website" @click.stop="run('website', item)"><ArrowUpRight :size="17"/></button><CornerDownLeft v-if="selected === index" :size="15" class="quick-enter"/>
          </div>
          <div v-if="!results.length" class="quick-empty"><Search :size="27"/><h3>No paths found.</h3><p>Try a name, domain, or a few letters.</p></div>
        </div>
        <div v-if="actions && current" class="quick-actions" aria-label="Item actions"><p>ACTIONS FOR {{ current.name.toUpperCase() }}</p><button v-for="action in actionList" :key="action.action" @click="run(action.action)"><component :is="action.icon" :size="16"/>{{ action.label }}<kbd>{{ action.key }}</kbd></button><p class="action-hint">Filling requires the Veyra browser companion. The form is never submitted automatically.</p></div>
        <footer class="quick-footer"><span><BrandMark/><strong>Veyra</strong><span class="quick-footer-mode">Quick Access</span></span><span class="quick-footer-keys"><kbd>↑</kbd><kbd>↓</kbd><span>navigate</span><button @click="actions = !actions"><kbd>{{ prefs.shortcuts.quickActions }}</kbd> Actions <ChevronRight :size="13"/></button></span></footer>
      </template>
    </section>
  </div>
</template>

<style scoped>
.quick-open-button { flex-shrink: 0; display: grid; place-items: center; width: 30px; height: 30px; border-radius: 6px; color: var(--muted); }
.quick-open-button:hover, .quick-open-button:focus-visible { color: var(--accent); background: var(--accent-bg); }
</style>
