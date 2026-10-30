<script setup lang="ts">
import { X, Save, LoaderCircle } from '@lucide/vue'
import { invoke } from '@tauri-apps/api/core'
import { cardFields, identityFields, eraseItemDraft as erase } from '~/utils/itemFields'
import type { EditableItem, ItemDraft, VaultItem, WriteOutcome } from '~/types/vault'
const props = defineProps<{ item?: VaultItem; organizationId?: string | null }>()
const emit = defineEmits<{ close: []; saved: [id: string] }>()
const vault = useVault()
const draft = ref<ItemDraft | null>(null)
const busy = ref(true)
const error = ref('')
const panel = ref<HTMLElement>()
const generation = vault.session.value.generation
let disposed = false
let previousFocus: HTMLElement | null = null
const collections = computed(() => vault.collections.value.filter(collection => collection.organizationId === draft.value?.organizationId && !collection.readOnly && !collection.hidePasswords))
function close() { erase(draft.value); draft.value = null; emit('close') }
function onKey(event: KeyboardEvent) {
  if (event.key === 'Escape') { event.stopPropagation(); if (!busy.value) close() }
  if (event.key === 'Tab' && panel.value) {
    const nodes = Array.from(panel.value.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled])'))
    const first = nodes[0], last = nodes[nodes.length - 1]
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus() }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus() }
  }
}
watch(() => vault.session.value.generation, value => { if (value !== generation) close() }, { flush: 'sync' })
watch(vault.locked, value => { if (value) close() }, { flush: 'sync' })
onMounted(async () => {
  previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
  panel.value?.focus()
  try {
    if (!vault.connected.value) throw new Error('Editing requires a connected desktop vault.')
    if (props.item) {
      const result = await invoke<EditableItem>('get_editable_item', { itemId: props.item.id })
      if (disposed || vault.locked.value || result.generation !== generation || vault.session.value.generation !== generation) { erase(result.draft); return }
      draft.value = result.draft
    } else {
      draft.value = { id: null, revision: null, organizationId: props.organizationId || null, collectionIds: [], kind: 'login', name: '', favorite: false, notes: '', username: '', password: '', totp: '', uri: '', card: {}, identity: {} }
    }
    busy.value = false
    await nextTick(); panel.value?.querySelector<HTMLInputElement>('[name="item-name"]')?.focus()
  } catch (failure) { if (!disposed) error.value = vault.errorMessage(failure, 'Could not open the editor.') }
  finally { busy.value = false }
})
onBeforeUnmount(() => { disposed = true; erase(draft.value); draft.value = null; previousFocus?.focus() })
async function submit() {
  if (!draft.value || busy.value || vault.locked.value || vault.needsSync.value) return
  busy.value = true; error.value = ''
  // Only this component holds editor fields. No useState/store/storage receives them.
  const request = { ...draft.value, collectionIds: [...draft.value.collectionIds], card: draft.value.card ? { ...draft.value.card } : null, identity: draft.value.identity ? { ...draft.value.identity } : null }
  try {
    const outcome = await invoke<WriteOutcome>('save_item', { expectedGeneration: generation, draft: request })
    if (!disposed && vault.acceptWrite(outcome)) { emit('saved', outcome.itemId); close() }
  } catch (failure) { if (!disposed) error.value = vault.errorMessage(failure, 'Could not save this item.') }
  finally { erase(request); busy.value = false }
}
</script>
<template>
  <div class="modal-backdrop editor-backdrop" @click.self="!busy && close()" @keydown="onKey">
    <section ref="panel" class="item-editor" tabindex="-1" role="dialog" aria-modal="true" aria-labelledby="editor-heading">
      <header><h2 id="editor-heading">{{ item ? 'Edit item' : 'New item' }}</h2><button class="icon-button" aria-label="Close editor" :disabled="busy" @click="close"><X :size="20"/></button></header>
      <p v-if="busy && !draft" role="status">Loading editor…</p>
      <div v-if="error" class="auth-error" role="alert">{{ error }}</div>
      <p v-if="vault.needsSync.value" class="compatibility-notice">Synchronize and inspect the vault before another write. Close this editor to refresh.</p>
      <form v-if="draft" autocomplete="off" @submit.prevent="submit">
        <fieldset :disabled="busy || vault.needsSync.value">
          <label class="connection-field"><span>Name</span><input v-model="draft.name" name="item-name" required maxlength="1024" autocomplete="off"></label>
          <label class="connection-field"><span>Item type</span><select v-model="draft.kind" :disabled="!!item"><option value="login">Login</option><option value="note">Secure note</option><option value="card">Card</option><option value="identity">Identity</option></select></label>
          <label class="connection-field"><span>Vault</span><select v-model="draft.organizationId" :disabled="!!item" @change="draft.collectionIds = []"><option :value="null">Your vault</option><option v-for="org in vault.organizations.value" :key="org.id" :value="org.id">{{ org.name }}</option></select></label>
          <div v-if="draft.organizationId && !item" class="editor-collections"><p>Collections · select at least one</p><label v-for="collection in collections" :key="collection.id"><input v-model="draft.collectionIds" type="checkbox" :value="collection.id">{{ collection.name }}</label><p v-if="!collections.length">No writable collections available.</p></div>
          <template v-if="draft.kind === 'login'">
            <label class="connection-field"><span>Username</span><input v-model="draft.username" autocomplete="off" maxlength="4096" spellcheck="false"></label>
            <label class="connection-field"><span>Password</span><input v-model="draft.password" type="password" autocomplete="new-password" maxlength="4096"></label>
            <label class="connection-field"><span>Website · first URI</span><input v-model="draft.uri" autocomplete="off" maxlength="8192" spellcheck="false"></label>
            <label class="connection-field"><span>Authenticator secret</span><input v-model="draft.totp" type="password" autocomplete="off" maxlength="4096" spellcheck="false"></label>
          </template>
          <div v-if="draft.kind === 'card' && draft.card" class="editor-fields"><label v-for="field in cardFields" :key="field.key" class="connection-field"><span>{{ field.label }}</span><input v-model="draft.card[field.key]" :type="field.key === 'code' || field.key === 'number' ? 'password' : 'text'" autocomplete="off" maxlength="4096"></label></div>
          <div v-if="draft.kind === 'identity' && draft.identity" class="editor-fields"><label v-for="field in identityFields" :key="field.key" class="connection-field"><span>{{ field.label }}</span><input v-model="draft.identity[field.key]" autocomplete="off" maxlength="4096"></label></div>
          <label class="connection-field"><span>Notes</span><textarea v-model="draft.notes" rows="4" maxlength="65536" spellcheck="false"/></label>
          <label class="editor-favorite"><input v-model="draft.favorite" type="checkbox">Favorite</label>
        </fieldset>
        <p class="setting-hint">Saved encrypted to Vaultwarden. Existing custom fields, extra URIs and attachments are preserved. Moving existing items between vaults is unavailable.</p>
        <footer><button type="button" class="secondary-button" :disabled="busy" @click="close">Cancel</button><button type="submit" class="primary-button" :disabled="busy || vault.needsSync.value || (!!draft.organizationId && !item && !draft.collectionIds.length)"><LoaderCircle v-if="busy" :size="16" class="spin"/><Save v-else :size="16"/>{{ busy ? 'Saving…' : 'Save item' }}</button></footer>
      </form>
    </section>
  </div>
</template>
