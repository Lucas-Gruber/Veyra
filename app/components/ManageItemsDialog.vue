<script setup lang="ts">
import { X } from '@lucide/vue'
import type { BatchOutcome, ManagedItemResult, ManagementOperation, VaultItem } from '~/types/vault'
export type ManagementMode = 'folder' | 'collections' | 'copy' | 'move' | 'trash' | 'restore'
const props = defineProps<{ items: readonly VaultItem[]; mode: ManagementMode }>()
const emit = defineEmits<{ close: [] }>()
const vault = useVault()
const panel = ref<HTMLElement>()
const closeButton = ref<HTMLButtonElement>()
const busy = ref(false)
const error = ref('')
const outcome = ref<Pick<BatchOutcome, 'results' | 'refreshPending'>>()
const sources = props.items.map(item => ({ ...item, collectionIds: [...(item.collectionIds || [])] }))
const generation = vault.session.value.generation
let alive = true
watch(() => vault.session.value.generation, value => { if (value !== generation) emit('close') }, { flush: 'sync' })
const sameOrganization = sources.every(item => item.organizationId === sources[0]?.organizationId)
const organizationId = ref(sameOrganization ? sources[0]?.organizationId || '' : '')
const folderId = ref(sources.every(item => item.folderId === sources[0]?.folderId) ? sources[0]?.folderId || '' : '')
const collectionIds = ref<string[]>(sameOrganization ? sources[0]?.collectionIds?.filter(id => sources.every(item => item.collectionIds?.includes(id))) || [] : [])
const collections = computed(() => vault.collections.value.filter(collection => collection.organizationId === organizationId.value && !collection.readOnly && !collection.hidePasswords))
watch(organizationId, () => { collectionIds.value = [] })
const title = computed(() => ({ folder: 'Assign folder', collections: 'Assign collections', copy: 'Duplicate items', move: 'Move to another vault', trash: 'Move to trash', restore: 'Restore items' })[props.mode])
const unavailable = computed(() => {
  if (!sources.length || !vault.connected.value) return 'Connect and unlock a vault first.'
  if (vault.needsSync.value) return 'Synchronize and inspect the vault before another change.'
  if (props.mode === 'trash') return sources.some(item => !item.canDelete || item.deleted) ? 'Some selected items cannot be moved to trash.' : ''
  if (props.mode === 'restore') return sources.some(item => !item.canRestore || !item.deleted) ? 'Some selected items cannot be restored.' : ''
  if (sources.some(item => item.readOnly || item.deleted)) return 'Some selected items are read-only or in trash.'
  if (props.mode === 'collections' && (!sameOrganization || !organizationId.value)) return 'Select items from one organization to assign their collections.'
  if (['collections', 'move', 'copy'].includes(props.mode) && organizationId.value && !collectionIds.value.length) return 'Choose at least one writable collection in the destination organization.'
  return ''
})
let previousFocus: HTMLElement | null = null
onMounted(() => { previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null; closeButton.value?.focus() })
onBeforeUnmount(() => { alive = false; if (previousFocus?.isConnected) previousFocus.focus() })
function close() { if (!busy.value) emit('close') }
function onKey(event: KeyboardEvent) {
  if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(); return }
  if (event.key !== 'Tab') return
  const fields = Array.from(panel.value?.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), select:not([disabled])') || [])
  const first = fields[0], last = fields.at(-1)
  if ((event.shiftKey && document.activeElement === first) || (!event.shiftKey && document.activeElement === last)) { event.preventDefault(); (event.shiftKey ? last : first)?.focus() }
}
async function apply() {
  if (busy.value || outcome.value || unavailable.value || vault.session.value.generation !== generation) return
  busy.value = true; error.value = ''
  try {
    if (props.mode === 'trash' || props.mode === 'restore') {
      const results: ManagedItemResult[] = []
      for (const item of sources) {
        if (!alive || vault.session.value.generation !== generation || vault.locked.value || vault.needsSync.value || results.some(result => result.status !== 'succeeded')) {
          results.push({ sourceId: item.id, itemId: null, status: 'not_attempted', error: null }); continue
        }
        const succeeded = await vault.changeItem(item, props.mode)
        results.push({ sourceId: item.id, itemId: succeeded ? item.id : null, status: succeeded ? 'succeeded' : vault.needsSync.value ? 'uncertain' : 'failed', error: succeeded ? null : { code: 'write_failed', message: vault.feedback.value || 'The change was not confirmed.' } })
      }
      outcome.value = { results, refreshPending: vault.needsSync.value }
    } else {
      const operation: ManagementOperation = props.mode === 'folder' ? { type: 'assign_folder', folderId: folderId.value || null }
        : props.mode === 'collections' ? { type: 'assign_collections', collectionIds: collectionIds.value }
          : { type: props.mode, organizationId: organizationId.value || null, collectionIds: organizationId.value ? collectionIds.value : [], folderId: folderId.value || null }
      outcome.value = await vault.manageItems(sources, operation) || undefined
      if (!outcome.value) error.value = vault.feedback.value || 'No result was confirmed. Synchronize and inspect the vault.'
    }
  } finally { busy.value = false }
}
</script>
<template>
  <div class="modal-backdrop" @click.self="close">
    <section ref="panel" class="management-dialog" role="dialog" aria-modal="true" :aria-label="title" @keydown="onKey">
      <header><h2 id="management-title">{{ title }}</h2><button ref="closeButton" class="icon-button" aria-label="Close item management" :disabled="busy" @click="close"><X :size="20"/></button></header>
      <p>{{ sources.length }} {{ sources.length === 1 ? 'selected item' : 'selected items' }}</p>
      <ul class="source-items"><li v-for="item in sources" :key="item.id">{{ item.name }}</li></ul>
      <template v-if="!outcome">
        <template v-if="mode === 'copy' || mode === 'move'">
          <label>Destination vault<select v-model="organizationId" :disabled="busy"><option value="">Your vault</option><option v-for="org in vault.organizations.value" :key="org.id" :value="org.id">{{ org.name }}</option></select></label>
          <p class="setting-hint">{{ mode === 'copy' ? 'Creates independent copies. The original items stay where they are.' : 'Moves the items to the destination vault. Members with access to its collections can read them.' }}</p>
        </template>
        <label v-if="['folder', 'copy', 'move'].includes(mode)">Folder<select v-model="folderId" :disabled="busy"><option value="">No folder</option><option v-for="folder in vault.folders.value" :key="folder.id" :value="folder.id">{{ folder.name || 'Unnamed folder' }}</option></select></label>
        <fieldset v-if="mode === 'collections' || (['copy', 'move'].includes(mode) && organizationId)" :disabled="busy"><legend>Destination collections</legend><label v-for="collection in collections" :key="collection.id" class="collection-choice"><input v-model="collectionIds" type="checkbox" :value="collection.id">{{ collection.name }}</label><p v-if="!collections.length" class="setting-hint">No writable collections are available in this organization.</p></fieldset>
        <p v-if="mode === 'trash'" class="setting-hint">These items will be moved to Vaultwarden’s trash. You can restore them later.</p>
        <p v-if="unavailable || error" class="auth-error" role="alert">{{ error || unavailable }}</p>
        <footer><button class="secondary-button" :disabled="busy" @click="close">Cancel</button><button class="primary-button" :disabled="busy || !!unavailable || vault.mutating.value" @click="apply">{{ busy ? 'Applying…' : `Apply to ${sources.length} ${sources.length === 1 ? 'item' : 'items'}` }}</button></footer>
      </template>
      <template v-else>
        <p role="status">{{ outcome.results.filter(result => result.status === 'succeeded').length }} of {{ sources.length }} completed.</p>
        <p v-if="outcome.refreshPending" class="auth-error" role="alert">Synchronize and inspect the vault before making another change.</p>
        <ul class="management-results"><li v-for="result in outcome.results" :key="result.sourceId"><strong>{{ sources.find(item => item.id === result.sourceId)?.name }}</strong><span>{{ result.status.replace('_', ' ') }}</span><p v-if="result.error">{{ result.error.message }}</p><p v-if="result.itemId && result.status !== 'succeeded'">A destination item may already exist: {{ result.itemId }}. Inspect it before retrying.</p></li></ul>
        <footer><button class="secondary-button" @click="emit('close')">Done</button></footer>
      </template>
    </section>
  </div>
</template>
<style scoped>
.management-dialog { width: min(560px, 100%); max-height: calc(100dvh - 48px); overflow: auto; padding: 24px; background: var(--panel); border: 1px solid var(--border-strong); border-radius: 14px; box-shadow: 0 20px 80px #0006; }
header,footer { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
h2 { font-size: 1.25rem; } p { margin: 12px 0; color: var(--muted); font-size: .875rem; }
label { display: block; margin: 16px 0; font-size: .875rem; }
select { display: block; width: 100%; margin-top: 8px; padding: 10px; color: var(--text); background: var(--surface); border: 1px solid var(--border-strong); border-radius: 6px; }
fieldset { border: 1px solid var(--border-strong); padding: 12px; border-radius: 8px; }
legend { padding: 0 5px; font-size: .875rem; }
.collection-choice { display: flex; align-items: center; gap: 10px; margin: 8px 0; }
.source-items { max-height: 100px; overflow: auto; padding-left: 20px; color: var(--muted); font-size: .8125rem; }
.management-results { padding: 0; list-style: none; font-size: .875rem; }
.management-results li { padding: 12px 0; border-bottom: 1px solid var(--border); overflow-wrap: anywhere; }
.management-results span { display: block; margin-top: 5px; color: var(--muted); }
footer { justify-content: flex-end; margin-top: 24px; }
</style>
