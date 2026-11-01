<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { defaultShortcuts, shortcutDefinitions, shortcutFromEvent, type ShortcutAction } from '~/utils/shortcuts'
const prefs = usePreferences()
const vault = useVault()
const recording = ref<ShortcutAction | null>(null)
const error = ref('')
const nativeStatus = ref<{ state: string; shortcut: string; message: string; command: string; trigger?: string | null }>()
let statusTimer: ReturnType<typeof setInterval> | undefined
let alive = true
let checking = false
async function refreshStatus() {
  if (!alive || checking || !vault.desktop.value) return
  checking = true
  try {
    const result = await invoke<typeof nativeStatus.value>('quick_access_shortcut_status')
    if (alive) nativeStatus.value = result
  } catch { /* The local bindings remain usable without desktop registration. */ }
  finally { checking = false }
}
async function configure(shortcut: string) {
  if (!vault.desktop.value) return true
  try {
    nativeStatus.value = await invoke('configure_quick_access_shortcut', { shortcut })
    const applied = nativeStatus.value?.state === 'unavailable' || (nativeStatus.value?.state === 'registered' && nativeStatus.value.shortcut === shortcut)
    if (!applied) error.value = nativeStatus.value?.message || 'The desktop shortcut was not changed.'
    return applied
  } catch (failure) { error.value = vault.errorMessage(failure, 'Could not configure the desktop shortcut.'); return false }
}
async function capture(event: KeyboardEvent, action: ShortcutAction) {
  if (recording.value !== action) return
  event.preventDefault(); event.stopPropagation()
  if (event.key === 'Escape') { recording.value = null; return }
  const key = shortcutFromEvent(event)
  if (!key) { error.value = 'Choose a letter, number, function key or navigation key, optionally with modifiers.'; return }
  if (/^[A-Z0-9,.;/[\]\\'`=-]$/.test(key)) { error.value = 'Use Ctrl, Alt or Meta with printable keys so typing stays available.'; return }
  if (Object.entries(prefs.shortcuts).some(([id, value]) => id !== action && value === key)) { error.value = 'This shortcut is already assigned. Choose another combination.'; return }
  if (action === 'openQuick' && !await configure(key)) return
  prefs.shortcuts = { ...prefs.shortcuts, [action]: key }; prefs.persist(); recording.value = null; error.value = ''
}
async function reset() {
  if (!await configure(defaultShortcuts.openQuick)) return
  prefs.shortcuts = { ...defaultShortcuts }; prefs.persist(); recording.value = null; error.value = ''
}
onMounted(() => { void refreshStatus(); statusTimer = setInterval(() => { if (nativeStatus.value?.state === 'pending') void refreshStatus() }, 1000) })
onBeforeUnmount(() => { alive = false; clearInterval(statusTimer) })
</script>
<template>
  <div class="shortcut-settings">
    <p class="setting-hint">Click a shortcut and press its new combination. Escape cancels recording. Selected input text keeps its normal copy shortcut.</p>
    <div v-if="error" role="alert" class="auth-error">{{ error }}</div>
    <div v-for="entry in shortcutDefinitions" :key="entry.id" class="setting-row"><span>{{ entry.label }}</span><button class="shortcut-recorder" :aria-label="`Change shortcut: ${entry.label}`" :aria-pressed="recording === entry.id" @click="recording = entry.id; error = ''" @keydown="capture($event, entry.id)">{{ recording === entry.id ? 'Press shortcut…' : prefs.shortcuts[entry.id] }}</button></div>
    <button class="secondary-button" @click="reset">Restore default shortcuts</button>
    <p v-if="nativeStatus" class="setting-hint" role="status">{{ nativeStatus.message }}<span v-if="nativeStatus.trigger"><br>Desktop binding: {{ nativeStatus.trigger }}</span><br>Manual desktop shortcut: <code>{{ nativeStatus.command }}</code></p>
    <p v-else class="setting-hint">Global registration depends on the desktop. The command <code>veyra --quick-access</code> can be assigned in your compositor.</p>
  </div>
</template>
<style scoped>
.shortcut-recorder { background: var(--surface); color: var(--text); border: 1px solid var(--border-strong); border-radius: 6px; padding: 7px 10px; font-size: .8125rem; min-width: 110px; }
.shortcut-recorder[aria-pressed=true] { border-color: var(--accent); color: var(--accent); }
.setting-row { gap: 18px; }
.shortcut-settings > .secondary-button { margin-top: 20px; }
</style>
