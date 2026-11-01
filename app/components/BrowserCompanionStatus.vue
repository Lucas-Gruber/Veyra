<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
const vault = useVault()
const status = ref<{ supported: boolean; connected: boolean; message: string }>()
const error = ref('')
const checking = ref(false)
let alive = true
async function check() {
  if (!vault.desktop.value || checking.value) return
  checking.value = true; error.value = ''
  try {
    const result = await invoke<typeof status.value>('browser_companion_status')
    if (alive) status.value = result
  } catch (failure) { if (alive) error.value = vault.errorMessage(failure, 'Browser companion status is unavailable.') }
  finally { checking.value = false }
}
onMounted(() => { void check() })
onBeforeUnmount(() => { alive = false })
</script>
<template>
  <div>
    <h4>Browser companion</h4>
    <p class="setting-hint">Filling requires the Veyra companion extension and its native messaging host in your default browser. It uses the saved HTTPS address and never submits the form.</p>
    <p class="setting-hint" role="status">{{ status?.message || error || (vault.desktop.value ? 'Checking companion…' : 'Available with a connected native Linux vault.') }}</p>
    <button v-if="vault.desktop.value" class="secondary-button" :disabled="checking" @click="check">{{ checking ? 'Checking…' : 'Check companion' }}</button>
  </div>
</template>
