<script setup lang="ts">
import { ArrowUpRight, Copy, Eye, EyeOff, Star, LockKeyhole, Globe, Clock3, FolderClosed, ShieldCheck, Pencil, Trash2, Undo2 } from '@lucide/vue'
import type { VaultItem } from '~/types/vault'
const props = defineProps<{ item: VaultItem }>()
const emit = defineEmits<{ edit: [] }>()
const vault = useVault()
const revealed = ref('')
const confirmTrash = ref(false)
const canPassword = computed(() => vault.demo.value || props.item.hasPassword !== false)
const canTotp = computed(() => vault.demo.value || props.item.hasTotp !== false)
let generation = 0
let revealTimer: ReturnType<typeof setTimeout> | undefined
function conceal() { generation++; revealed.value = ''; clearTimeout(revealTimer) }
async function toggleReveal() {
  if (revealed.value) conceal()
  else {
    const itemId = props.item.id
    const requestGeneration = generation
    const result = await vault.reveal(itemId)
    if (itemId === props.item.id && !vault.locked.value && requestGeneration === generation) {
      revealed.value = result
      revealTimer = setTimeout(conceal, 10000)
    }
  }
}
watch(() => props.item.revision, () => { conceal(); confirmTrash.value = false })
watch(() => props.item.id, conceal)
watch(vault.locked, conceal)
onBeforeUnmount(conceal)
</script>
<template>
  <article class="item-detail" aria-labelledby="detail-name">
    <header class="detail-toolbar"><span><LockKeyhole :size="13"/> {{ vault.demo.value ? 'DEMONSTRATION ITEM' : item.deleted ? 'IN TRASH' : item.readOnly ? 'READ-ONLY ITEM' : 'VAULTWARDEN ITEM' }}</span><button class="icon-button" :class="{ 'is-favorite': item.favorite }" :aria-label="item.favorite ? 'Remove from favorites' : 'Add to favorites'" :aria-pressed="item.favorite" :disabled="!vault.demo.value && (item.readOnly || vault.mutating.value || vault.needsSync.value)" @click="vault.toggleFavorite(item.id)"><Star :size="18" :fill="item.favorite ? 'currentColor' : 'none'"/></button></header>
    <div class="detail-identity"><ItemIcon :item="item" large/><h2 id="detail-name">{{ item.name }}</h2><button v-if="item.url && !item.deleted" class="domain-link" @click="vault.openWebsite(item)">{{ item.domain }} <ArrowUpRight :size="13"/></button></div>
    <div v-if="vault.connected.value" class="detail-edit-actions"><button v-if="!item.deleted && !item.readOnly" class="secondary-button" :disabled="vault.mutating.value || vault.needsSync.value" @click="emit('edit')"><Pencil :size="15"/>Edit item</button><button v-if="!item.deleted && item.canDelete" class="secondary-button" :disabled="vault.mutating.value || vault.needsSync.value" @click="confirmTrash = true"><Trash2 :size="15"/>Move to trash</button><button v-if="item.deleted && item.canRestore" class="secondary-button" :disabled="vault.mutating.value || vault.needsSync.value" @click="vault.changeItem(item, 'restore')"><Undo2 :size="15"/>Restore item</button></div>
    <div v-if="confirmTrash" class="trash-confirmation" role="alert"><p>Move {{ item.name }} to Trash in Vaultwarden?</p><button class="secondary-button" :disabled="vault.mutating.value" @click="confirmTrash = false">Cancel</button><button class="secondary-button" :disabled="vault.mutating.value || vault.needsSync.value" @click="vault.changeItem(item, 'trash')">Confirm move to trash</button></div>
    <div v-if="item.kind === 'login' && !item.deleted" class="detail-actions"><UButton v-if="item.url" class="primary-button" @click="vault.openWebsite(item)"><ArrowUpRight :size="16"/>Open website</UButton><button class="secondary-button" :disabled="!canPassword" @click="vault.copy(item.id, 'password')"><Copy :size="15"/>Copy password</button></div>
    <section v-if="item.kind === 'login' && !item.deleted" class="fields-panel" aria-label="Login information">
      <div class="field-row"><div><label>Username</label><span>{{ item.username }}</span></div><button class="icon-button" aria-label="Copy username" @click="vault.copy(item.id, 'username')"><Copy :size="16"/></button></div>
      <div class="field-row"><div><label>Password</label><span :class="['password-value', { concealed: !revealed }]">{{ revealed || '••••••••••••••••' }}</span></div><button class="icon-button" :disabled="!canPassword" :aria-label="revealed ? 'Hide password' : vault.demo.value ? 'Reveal sample password' : 'Reveal password'" @click="toggleReveal"><EyeOff v-if="revealed" :size="17"/><Eye v-else :size="17"/></button><button class="icon-button" aria-label="Copy password" :disabled="!canPassword" @click="vault.copy(item.id, 'password')"><Copy :size="16"/></button></div>
      <div class="field-row"><div><label>One-time password <span v-if="vault.demo.value" class="tiny-chip">SAMPLE</span></label><span class="code-mask">••• •••</span></div><button class="icon-button" :disabled="!canTotp" :aria-label="vault.demo.value ? 'Copy sample TOTP' : 'Copy one-time password'" @click="vault.copy(item.id, 'totp')"><Copy :size="16"/></button></div>
      <div v-if="item.url" class="field-row"><div><label>Website</label><button class="field-link" @click="vault.openWebsite(item)"><Globe :size="14"/>{{ item.url }}</button></div><button class="icon-button" aria-label="Open website" @click="vault.openWebsite(item)"><ArrowUpRight :size="17"/></button></div>
    </section>
    <section v-if="vault.demo.value" class="detail-notes"><h3>Notes</h3><p>{{ item.note || 'A little space for the details that matter.' }}</p></section>
    <ItemContents v-else-if="!item.deleted" :item="item"/>
    <div class="detail-tags"><span v-for="tag in item.tags" :key="tag">{{ tag }}</span></div>
    <footer class="detail-footer"><span><FolderClosed :size="14"/>{{ item.folder }}</span><span><Clock3 :size="14"/>Updated {{ item.updated.toLowerCase() }}</span></footer>
    <div class="security-caption"><ShieldCheck :size="14"/>{{ vault.demo.value ? 'Sample data only. Your real vault stays yours.' : 'Secrets are accessed explicitly. Nothing is saved offline.' }}</div>
  </article>
</template>
