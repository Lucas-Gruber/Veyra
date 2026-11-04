<script setup lang="ts">
import { ArrowRight, Fingerprint, ShieldCheck, LoaderCircle, Monitor, AlertCircle } from '@lucide/vue'
import type { TwoFactorProvider } from '~/types/vault'
import type { LoginPreferences } from '~/composables/useLoginPreferences'
const vault = useVault()
const preferences = useLoginPreferences()
defineProps<{ compact?: boolean }>()
const serverUrl = ref('')
const email = ref('')
const masterPassword = ref('')
const twoFactorCode = ref('')
const providers = ref<TwoFactorProvider[]>([])
const provider = ref<TwoFactorProvider>('totp')
const busy = ref(false)
const error = ref('')
const preferenceError = ref('')
const preferenceLoading = ref(false)
const savedDetails = ref<LoginPreferences | null>(null)
const serverInput = ref<HTMLInputElement>()
const emailInput = ref<HTMLInputElement>()
const passwordInput = ref<HTMLInputElement>()
const demoButton = ref<HTMLButtonElement>()
let submittedGeneration = 0
let preferenceGeneration = 0
let focusGeneration = 0
let mounted = false
let lastPrefill: LoginPreferences = { serverUrl: '', email: '' }
let pendingLoad: Promise<void> | undefined
const stopListening: (() => void)[] = []
function clearSensitive() { masterPassword.value = ''; twoFactorCode.value = '' }
function clearForm() {
  preferenceGeneration++; focusGeneration++
  clearSensitive(); serverUrl.value = ''; email.value = ''; providers.value = []; error.value = ''; preferenceError.value = ''
  savedDetails.value = null; lastPrefill = { serverUrl: '', email: '' }; preferenceLoading.value = false
}
function loadPreferences() {
  if (!mounted || !vault.desktop.value) return Promise.resolve()
  const generation = ++preferenceGeneration
  const before = { serverUrl: serverUrl.value, email: email.value }
  const pristine = before.serverUrl === lastPrefill.serverUrl && before.email === lastPrefill.email
  preferenceLoading.value = true; preferenceError.value = ''
  const task = (async () => {
    try {
      const saved = await preferences.load()
      if (!mounted || generation !== preferenceGeneration) return
      savedDetails.value = saved
      // A response from another window must not replace an in-progress draft.
      if (pristine && before.serverUrl === serverUrl.value && before.email === email.value) {
        lastPrefill = saved ? { ...saved } : { serverUrl: '', email: '' }
        serverUrl.value = lastPrefill.serverUrl; email.value = lastPrefill.email
      }
    } catch (failure) {
      if (mounted && generation === preferenceGeneration) preferenceError.value = vault.errorMessage(failure, 'Saved sign-in details could not be loaded. Enter them below.')
    } finally { if (generation === preferenceGeneration) preferenceLoading.value = false }
  })()
  pendingLoad = task
  void task.finally(() => { if (pendingLoad === task) pendingLoad = undefined })
  return task
}
function focus() {
  const generation = ++focusGeneration
  void (async () => {
    while (pendingLoad) { const task = pendingLoad; await task; if (!mounted || generation !== focusGeneration) return }
    if (!mounted || generation !== focusGeneration) return
    await nextTick()
    if (!mounted || generation !== focusGeneration) return
    const target = !vault.desktop.value ? demoButton.value : !serverUrl.value ? serverInput.value : !email.value ? emailInput.value : passwordInput.value
    target?.focus({ preventScroll: true })
  })()
}
defineExpose({ clear: () => { submittedGeneration++; clearForm() }, focus })
watch(vault.locked, () => { submittedGeneration++; clearForm() })
onMounted(async () => {
  mounted = true
  window.addEventListener('focus', loadPreferences)
  void loadPreferences()
  if (vault.desktop.value) {
    const { listen } = await import('@tauri-apps/api/event')
    for (const event of ['login-preferences:changed', 'quick-access:focus']) {
      if (!mounted) return
      const stop = await listen(event, () => { void loadPreferences() })
      if (!mounted) stop()
      else stopListening.push(stop)
    }
  }
})
onBeforeUnmount(() => { mounted = false; submittedGeneration++; clearForm(); window.removeEventListener('focus', loadPreferences); stopListening.forEach(stop => stop()) })
async function submit() {
  if (busy.value || preferenceLoading.value || !vault.desktop.value) return
  error.value = ''
  busy.value = true
  const generation = ++submittedGeneration
  const request = {
    serverUrl: serverUrl.value.trim(),
    email: email.value.trim(),
    masterPassword: masterPassword.value,
    ...(providers.value.length ? { twoFactor: { provider: provider.value, code: twoFactorCode.value.trim() } } : {}),
  }
  clearSensitive()
  try {
    try {
      const saved = await preferences.save({ serverUrl: request.serverUrl, email: request.email })
      if (!mounted || generation !== submittedGeneration) return
      savedDetails.value = saved; lastPrefill = { ...saved }
      serverUrl.value = saved.serverUrl; email.value = saved.email
      request.serverUrl = saved.serverUrl; request.email = saved.email
    } catch (failure) {
      if (mounted && generation === submittedGeneration) preferenceError.value = vault.errorMessage(failure, 'Sign-in details could not be remembered. You can still connect.')
    }
    if (!mounted || generation !== submittedGeneration) return
    const outcome = await vault.login(request)
    if (generation !== submittedGeneration) return
    if (outcome.status === 'two_factor_required') {
      providers.value = outcome.providers
      provider.value = outcome.providers[0] || 'totp'
      if (!outcome.providers.length) error.value = 'This account requires an authentication method this version does not support.'
      await nextTick()
      if (mounted && generation === submittedGeneration) passwordInput.value?.focus({ preventScroll: true })
    }
  } catch (failure) {
    if (generation === submittedGeneration) error.value = vault.errorMessage(failure, 'Connection failed. Check the server address and sign-in details.')
  } finally {
    request.masterPassword = ''
    if (request.twoFactor) request.twoFactor.code = ''
    busy.value = false
  }
}
async function forgetDetails() {
  if (busy.value || preferenceLoading.value) return
  clearSensitive()
  busy.value = true; preferenceError.value = ''
  const generation = ++submittedGeneration
  try {
    await preferences.clear()
    if (mounted && generation === submittedGeneration) clearForm()
  } catch (failure) {
    if (mounted && generation === submittedGeneration) preferenceError.value = vault.errorMessage(failure, 'Saved sign-in details could not be forgotten. Try again.')
  } finally { busy.value = false }
}
async function exploreDemo() { clearForm(); await vault.unlock() }
</script>
<template>
  <section class="locked-vault connection-view" :class="{ compact }">
    <div class="lock-orbit"><span/><Fingerprint :size="compact ? 30 : 38" :stroke-width="1.35"/></div>
    <p class="eyebrow">YOUR VAULT. YOUR SERVER.</p>
    <h1>{{ providers.length ? 'One more step.' : 'Welcome to Veyra.' }}</h1>
    <p class="locked-copy">{{ vault.desktop.value ? 'Connect securely to your Vaultwarden server.' : 'A thoughtful home for your digital life.' }}</p>
    <form v-if="vault.desktop.value" class="connection-form" @submit.prevent="submit">
      <template v-if="providers.length"><p class="challenge-note">Your server requires two-factor authentication. Re-enter your master password and verification code; neither is retained after submission.</p><label v-if="providers.length > 1" class="connection-field"><span>Verification method</span><select v-model="provider" :disabled="busy"><option v-for="option in providers" :key="option" :value="option">{{ option === 'totp' ? 'Authenticator app' : 'Email verification' }}</option></select></label></template>
      <label class="connection-field"><span>Server address</span><input ref="serverInput" v-model="serverUrl" type="url" placeholder="https://vault.example.com" autocomplete="off" autocapitalize="off" spellcheck="false" required :disabled="busy || preferenceLoading"></label>
      <label class="connection-field"><span>Email address</span><input ref="emailInput" v-model="email" type="email" placeholder="you@example.com" autocomplete="off" autocapitalize="off" spellcheck="false" required :disabled="busy || preferenceLoading"></label>
      <label class="connection-field"><span>Master password</span><input ref="passwordInput" v-model="masterPassword" type="password" autocomplete="off" placeholder="Your master password" required :disabled="busy || preferenceLoading"></label>
      <label v-if="providers.length" class="connection-field"><span>{{ provider === 'totp' ? 'Authenticator code' : 'Email verification code' }}</span><input v-model="twoFactorCode" inputmode="numeric" autocomplete="off" placeholder="Verification code" required :disabled="busy"></label>
      <div v-if="error" class="auth-error" role="alert"><AlertCircle :size="15"/><span>{{ error }}</span></div>
      <UButton type="submit" class="primary-button unlock-button connect-button" size="lg" :disabled="busy || preferenceLoading"><LoaderCircle v-if="busy" :size="16" class="spin"/><span>{{ busy ? 'Connecting securely…' : providers.length ? 'Verify & unlock' : 'Connect & unlock' }}</span><ArrowRight v-if="!busy" :size="16"/></UButton>
      <button v-if="providers.length" type="button" class="text-button" :disabled="busy" @click="providers = []; clearSensitive(); error = ''">Back to sign in</button>
      <p class="connection-security"><ShieldCheck :size="13"/>Vault keys and sessions stay in Rust.</p>
      <p class="setting-hint">Submitting remembers your server address and email on this device. Passwords and verification codes are never saved.</p>
      <p v-if="preferenceError" class="setting-hint" role="status">{{ preferenceError }}</p>
      <button v-if="savedDetails" type="button" class="text-button" :disabled="busy || preferenceLoading" @click="forgetDetails">Forget saved sign-in details</button>
      <p class="connection-limit">Integration preview. Locking signs you out; offline unlock is not available. Independent security review is pending.</p>
    </form>
    <div v-else class="browser-connect-notice"><Monitor :size="22"/><strong>Connect in the desktop application</strong><p>Vaultwarden authentication and decryption run in Rust. This browser preview can only show synthetic sample data.</p></div>
    <div class="demo-entry"><span>{{ vault.desktop.value ? 'Just looking around?' : 'Explore the interface' }}</span><button ref="demoButton" class="text-button" :disabled="busy" @click="exploreDemo">Explore demo vault <ArrowRight :size="13"/></button></div>
    <div v-if="!compact" class="locked-tagline">Your vault. One shortcut away.</div>
  </section>
</template>
