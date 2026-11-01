<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
const vault = useVault()
const status = ref<{ tray: 'ready' | 'host_unavailable' | 'unavailable'; message: string }>()
const error = ref('')
const checking = ref(false)
let alive = true
async function refresh() {
  if (!vault.desktop.value || checking.value) return
  checking.value = true; error.value = ''
  try {
    const result = await invoke<typeof status.value>('desktop_integration_status')
    if (alive) status.value = result
  } catch (failure) { if (alive) error.value = vault.errorMessage(failure, 'Could not check system tray availability.') }
  finally { checking.value = false }
}
onMounted(() => { void refresh() })
onBeforeUnmount(() => { alive = false })
</script>
<template>
  <div v-if="vault.desktop.value" class="desktop-integration-status">
    <h4>Background and system tray</h4>
    <p class="setting-hint">Closing Veyra keeps it running. Use the tray menu to reopen it, open Quick Access, lock the vault or quit.</p>
    <p class="setting-hint" role="status">{{ status?.message || error || 'Checking system tray…' }}</p>
    <p v-if="status?.tray !== 'ready'" class="setting-hint">You can also reopen the main window with <code>veyra</code>, or press your Open Veyra shortcut in Quick Access.</p>
    <button class="secondary-button" :disabled="checking" @click="refresh">{{ checking ? 'Checking…' : 'Check system tray' }}</button>
  </div>
</template>
