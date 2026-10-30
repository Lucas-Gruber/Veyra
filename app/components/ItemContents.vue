<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import type { EditableItem, ItemDraft, VaultItem } from '~/types/vault'
import { cardFields, identityFields, eraseItemDraft } from '~/utils/itemFields'
const props = defineProps<{ item: VaultItem }>()
const vault = useVault()
const contents = ref<ItemDraft | null>(null)
const busy = ref(false)
const error = ref('')
let epoch = 0
let disposed = false
let expiry: ReturnType<typeof setTimeout> | undefined
function conceal() { epoch++; clearTimeout(expiry); eraseItemDraft(contents.value); contents.value = null; error.value = '' }
async function toggle() {
  if (contents.value) { conceal(); return }
  if (busy.value) return
  const requestEpoch = ++epoch
  const generation = vault.session.value.generation
  busy.value = true; error.value = ''
  try {
    const result = await invoke<EditableItem>('get_item_contents', { itemId: props.item.id })
    if (disposed || requestEpoch !== epoch || vault.locked.value || result.generation !== generation || vault.session.value.generation !== generation) { eraseItemDraft(result.draft); return }
    contents.value = result.draft
    expiry = setTimeout(conceal, 60_000)
  } catch (failure) { if (!disposed && epoch === requestEpoch) error.value = vault.errorMessage(failure, 'Could not load the item contents.') }
  finally { busy.value = false }
}
watch(() => props.item.revision, conceal)
watch(vault.locked, conceal, { flush: 'sync' })
watch(() => vault.session.value.generation, conceal, { flush: 'sync' })
onMounted(() => window.addEventListener('blur', conceal))
onBeforeUnmount(() => { disposed = true; conceal(); window.removeEventListener('blur', conceal) })
</script>
<template>
  <section class="detail-notes item-contents">
    <h3>{{ item.kind === 'login' || item.kind === 'note' ? 'Notes' : 'Contents' }}</h3>
    <button class="secondary-button" :disabled="busy" @click="toggle">{{ busy ? 'Loading…' : contents ? 'Hide contents' : 'Show contents' }}</button>
    <p v-if="error" role="alert">{{ error }}</p>
    <template v-if="contents">
      <dl v-if="contents.card && item.kind === 'card'"><template v-for="field in cardFields" :key="field.key"><dt>{{ field.label }}</dt><dd>{{ contents.card[field.key] || '—' }}</dd></template></dl>
      <dl v-if="contents.identity && item.kind === 'identity'"><template v-for="field in identityFields" :key="field.key"><dt>{{ field.label }}</dt><dd>{{ contents.identity[field.key] || '—' }}</dd></template></dl>
      <p class="revealed-notes">{{ contents.notes || 'No notes.' }}</p>
      <p class="setting-hint">Contents hide after 60 seconds or when Veyra loses focus.</p>
    </template>
  </section>
</template>
