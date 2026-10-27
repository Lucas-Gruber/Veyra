<script setup lang="ts">
import { ArrowRight, Fingerprint, ShieldCheck, LoaderCircle, Monitor, AlertCircle } from '@lucide/vue'
import type { TwoFactorProvider } from '~/types/vault'
const vault = useVault()
defineProps<{ compact?: boolean }>()
const serverUrl = ref('')
const email = ref('')
const masterPassword = ref('')
const twoFactorCode = ref('')
const providers = ref<TwoFactorProvider[]>([])
const provider = ref<TwoFactorProvider>('totp')
const busy = ref(false)
const error = ref('')
const passwordInput = ref<HTMLInputElement>()
let submittedGeneration = 0
function clearSensitive() { masterPassword.value = ''; twoFactorCode.value = '' }
function clearForm() { clearSensitive(); serverUrl.value = ''; email.value = ''; providers.value = []; error.value = '' }
watch(vault.locked, () => { submittedGeneration++; clearForm() })
onBeforeUnmount(() => { submittedGeneration++; clearForm() })
async function submit() {
  if (busy.value || !vault.desktop.value) return
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
    const outcome = await vault.login(request)
    if (generation !== submittedGeneration) return
    if (outcome.status === 'two_factor_required') {
      providers.value = outcome.providers
      provider.value = outcome.providers[0] || 'totp'
      if (!outcome.providers.length) error.value = 'This account requires an authentication method this version does not support.'
      await nextTick()
      passwordInput.value?.focus()
    }
  } catch (failure) {
    if (generation === submittedGeneration) error.value = vault.errorMessage(failure, 'Connection failed. Check the server address and sign-in details.')
  } finally {
    request.masterPassword = ''
    if (request.twoFactor) request.twoFactor.code = ''
    busy.value = false
  }
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
      <label class="connection-field"><span>Server address</span><input v-model="serverUrl" type="url" placeholder="https://vault.example.com" autocomplete="off" autocapitalize="off" spellcheck="false" required :disabled="busy"></label>
      <label class="connection-field"><span>Email address</span><input v-model="email" type="email" placeholder="you@example.com" autocomplete="off" autocapitalize="off" spellcheck="false" required :disabled="busy"></label>
      <label class="connection-field"><span>Master password</span><input ref="passwordInput" v-model="masterPassword" type="password" autocomplete="off" placeholder="Your master password" required :disabled="busy"></label>
      <label v-if="providers.length" class="connection-field"><span>{{ provider === 'totp' ? 'Authenticator code' : 'Email verification code' }}</span><input v-model="twoFactorCode" inputmode="numeric" autocomplete="off" placeholder="Verification code" required :disabled="busy"></label>
      <div v-if="error" class="auth-error" role="alert"><AlertCircle :size="15"/><span>{{ error }}</span></div>
      <UButton type="submit" class="primary-button unlock-button connect-button" size="lg" :disabled="busy"><LoaderCircle v-if="busy" :size="16" class="spin"/><span>{{ busy ? 'Connecting securely…' : providers.length ? 'Verify & unlock' : 'Connect & unlock' }}</span><ArrowRight v-if="!busy" :size="16"/></UButton>
      <button v-if="providers.length" type="button" class="text-button" :disabled="busy" @click="providers = []; clearSensitive(); error = ''">Back to sign in</button>
      <p class="connection-security"><ShieldCheck :size="13"/>Vault keys and sessions stay in Rust. Sign-in details are not saved.</p>
      <p class="connection-limit">Read-only integration preview. Locking signs you out; offline unlock is not available. Independent security review is pending.</p>
    </form>
    <div v-else class="browser-connect-notice"><Monitor :size="22"/><strong>Connect in the desktop application</strong><p>Vaultwarden authentication and decryption run in Rust. This browser preview can only show synthetic sample data.</p></div>
    <div class="demo-entry"><span>{{ vault.desktop.value ? 'Just looking around?' : 'Explore the interface' }}</span><button class="text-button" :disabled="busy" @click="exploreDemo">Explore demo vault <ArrowRight :size="13"/></button></div>
    <div v-if="!compact" class="locked-tagline">Your vault. One shortcut away.</div>
  </section>
</template>
