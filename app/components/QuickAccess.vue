<script setup lang="ts">
import { Search, ArrowUpRight, KeyRound, Star, CornerDownLeft, ChevronRight, X, ExternalLink, UserRound, Clock3 } from '@lucide/vue'
import type { VaultItem } from '~/types/vault'
import { highlightSegments, searchItems } from '~/utils/search'
const props = defineProps<{ standalone?: boolean }>()
const emit = defineEmits<{ close: []; openItem: [id: string] }>()
const vault = useVault()
const prefs = usePreferences()
const query = ref('')
const selected = ref(0)
const actions = ref(false)
const searchInput = ref<HTMLInputElement>()
const dialog = ref<HTMLElement>()
let previousFocus: HTMLElement | null = null
let unlistenFocus: (() => void) | undefined
const results = computed(() => vault.locked.value ? [] : searchItems(vault.items.value, query.value).slice(0, 8))
const current = computed(() => results.value[selected.value])
watch(query, () => { selected.value = 0; actions.value = false })
watch(vault.locked, value => {
  if (value) { query.value = ''; selected.value = 0; actions.value = false }
  else nextTick(() => searchInput.value?.focus())
})
watch(selected, () => nextTick(() => document.querySelector('.quick-result.selected')?.scrollIntoView({ block: 'nearest' })))
onMounted(async () => {
  previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
  await nextTick()
  if (vault.locked.value) { const target = dialog.value?.querySelector<HTMLElement>('input, .demo-entry button'); target?.focus() }
  else searchInput.value?.focus()
  if (vault.desktop.value) {
    const { listen } = await import('@tauri-apps/api/event')
    unlistenFocus = await listen('quick-access:focus', () => nextTick(() => { if (vault.locked.value) dialog.value?.querySelector<HTMLInputElement>('input')?.focus(); else searchInput.value?.focus() }))
  }
})
onBeforeUnmount(() => { unlistenFocus?.(); previousFocus?.focus() })
async function close() {
  if (props.standalone) await vault.hideQuickAccess()
  else emit('close')
}
async function run(action: string, item: VaultItem | undefined = current.value) {
  if (!item) return
  if (item.kind !== 'login' && ['username', 'password', 'totp', 'website', 'autofill'].includes(action)) { vault.notify('This read-only version supports credential actions for login items only.'); return }
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
  if (action === 'website' || action === 'autofill') succeeded = await vault.openWebsite(item)
  else succeeded = await vault.copy(item.id, action as 'username' | 'password' | 'totp')
  if (!succeeded) return
  actions.value = false
  if (action === 'autofill') {
    // Keep this explanation visible, including inside the native floating window.
    vault.notify('Website opened. Autofill needs the future companion extension.')
    searchInput.value?.focus()
    return
  }
  try { await close() }
  catch { vault.notify('Action completed. Could not close Quick Access.') }
}
async function onKey(event: KeyboardEvent) {
  if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); if (actions.value) { actions.value = false; searchInput.value?.focus() } else await close(); return }
  if (vault.locked.value) {
    if (event.key === 'Tab') {
      const fields = Array.from(dialog.value?.querySelectorAll<HTMLElement>('input:not([disabled]), select:not([disabled]), button:not([disabled])') || [])
      const first = fields[0], last = fields[fields.length - 1]
      if ((event.shiftKey && document.activeElement === first) || (!event.shiftKey && document.activeElement === last)) { event.preventDefault(); (event.shiftKey ? last : first)?.focus() }
    }
    return
  }
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    if (actions.value) {
      const buttons = Array.from(dialog.value?.querySelectorAll<HTMLButtonElement>('.quick-actions button') || [])
      const index = buttons.indexOf(document.activeElement as HTMLButtonElement)
      const direction = event.key === 'ArrowDown' ? 1 : -1
      buttons[(index + direction + buttons.length) % buttons.length]?.focus()
      return
    }
    const direction = event.key === 'ArrowDown' ? 1 : -1
    selected.value = (selected.value + direction + results.value.length) % Math.max(1, results.value.length)
    return
  }
  if (event.key === 'Tab') {
    if (!actions.value) {
      event.preventDefault(); actions.value = true
      nextTick(() => {
        const buttons = Array.from(dialog.value?.querySelectorAll<HTMLButtonElement>('.quick-actions button') || [])
        ;(event.shiftKey ? buttons[buttons.length - 1] : buttons[0])?.focus()
      })
    } else {
      const buttons = Array.from(dialog.value?.querySelectorAll<HTMLButtonElement>('.quick-actions button') || [])
      if ((event.shiftKey && document.activeElement === buttons[0]) || (!event.shiftKey && document.activeElement === buttons[buttons.length - 1])) {
        event.preventDefault(); actions.value = false; searchInput.value?.focus()
      }
    }
    return
  }
  if (event.key === 'Enter' && !actions.value) { event.preventDefault(); await run(prefs.defaultAction); return }
  if (event.ctrlKey && !event.altKey) {
    const key = event.key.toLowerCase()
    // Ctrl+U remains standard text editing while the search input is active.
    if (key === 'u' && event.target === searchInput.value) return
    const action = ({ u: 'username', p: 'password', o: 'totp', b: event.shiftKey ? 'autofill' : 'website', e: 'detail', d: 'favorite' } as Record<string, string>)[key]
    if (action) { event.preventDefault(); await run(action) }
  }
}
const actionList = [
  { label: 'Copy password', action: 'password', key: 'Ctrl P', icon: KeyRound },
  { label: 'Copy username', action: 'username', key: 'Ctrl U', icon: UserRound },
  { label: 'Copy one-time code', action: 'totp', key: 'Ctrl O', icon: Clock3 },
  { label: 'Open website', action: 'website', key: 'Ctrl B', icon: ArrowUpRight },
  { label: 'Open & request autofill', action: 'autofill', key: 'Ctrl ⇧ B', icon: ExternalLink },
  { label: 'Open in Veyra', action: 'detail', key: 'Ctrl E', icon: ExternalLink },
  { label: 'Toggle favorite', action: 'favorite', key: 'Ctrl D', icon: Star },
]
</script>
<template>
  <div :class="standalone ? 'quick-standalone' : 'quick-backdrop'" @click.self="close">
    <section ref="dialog" class="quick-access" role="dialog" aria-modal="true" aria-label="Quick Access" @keydown="onKey">
      <template v-if="vault.locked.value"><button class="icon-button quick-close" aria-label="Close Quick Access" @click="close"><X :size="18"/></button><LockedVault compact/></template>
      <template v-else>
        <div class="quick-search"><Search :size="23" :stroke-width="1.7"/><input ref="searchInput" v-model="query" placeholder="Where do you want to go?" aria-label="Search vault" autocomplete="off" spellcheck="false" role="combobox" aria-autocomplete="list" aria-controls="quick-results" :aria-expanded="results.length > 0" :aria-activedescendant="current ? `quick-${current.id}` : undefined"><kbd @click="close">esc</kbd></div>
        <div class="quick-context"><span>{{ query ? `${results.length} MATCH${results.length === 1 ? '' : 'ES'}` : 'YOUR EVERYDAY ESSENTIALS' }}</span><span class="demo-dot">{{ vault.demo.value ? 'DEMO VAULT' : 'CONNECTED · READ-ONLY' }}</span></div>
        <div id="quick-results" class="quick-results" role="listbox" aria-label="Matching items">
          <button v-for="(item, index) in results" :id="`quick-${item.id}`" :key="item.id" :class="['quick-result', { selected: selected === index }]" role="option" :aria-selected="selected === index" @mousemove="selected = index" @click="run(prefs.defaultAction, item)">
            <ItemIcon :item="item"/><span class="quick-result-text"><strong><template v-for="(segment, i) in highlightSegments(item.name, query)" :key="i"><mark v-if="segment.match">{{ segment.text }}</mark><template v-else>{{ segment.text }}</template></template></strong><small>{{ item.username }}</small></span><span class="quick-domain">{{ item.domain }}</span><CornerDownLeft v-if="selected === index" :size="15" class="quick-enter"/>
          </button>
          <div v-if="!results.length" class="quick-empty"><Search :size="27"/><h3>No paths found.</h3><p>Try a name, domain, or a few letters.</p></div>
        </div>
        <div v-if="actions && current" class="quick-actions" aria-label="Item actions"><p>ACTIONS FOR {{ current.name.toUpperCase() }}</p><button v-for="action in actionList" :key="action.action" @click="run(action.action)"><component :is="action.icon" :size="16"/>{{ action.label }}<kbd>{{ action.key }}</kbd></button><p class="action-hint">Ctrl U edits text while searching; use the action menu to copy username. Autofill is planned; the website opens normally.</p></div>
        <footer class="quick-footer"><span><BrandMark/><strong>Veyra</strong><span class="quick-footer-mode">Quick Access</span></span><span class="quick-footer-keys"><kbd>↑</kbd><kbd>↓</kbd><span>navigate</span><button @click="actions = !actions"><kbd>tab</kbd> Actions <ChevronRight :size="13"/></button></span></footer>
      </template>
    </section>
  </div>
</template>
