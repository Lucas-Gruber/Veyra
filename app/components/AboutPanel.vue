<script setup lang="ts">
import { ArrowUpRight, CheckCircle2, Download, Info, LoaderCircle, RefreshCw, ShieldCheck } from '@lucide/vue'
const updates = useAppUpdates()
const prefs = usePreferences()
const lastSuccessfulCheck = computed(() => {
  const value = updates.result.value?.checkedAt
  if (!value) return 'Not checked yet'
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? 'Not checked yet' : date.toLocaleString()
})
const channelLabel = computed(() => !updates.isDesktop.value ? 'Browser preview' : updates.info.value?.channel === 'prerelease' ? 'Pre-release' : updates.info.value ? 'Stable' : 'Loading…')
const platformLabel = computed(() => {
  const info = updates.info.value
  if (!info) return updates.isDesktop.value ? 'Loading native information…' : 'Web preview · not an installed desktop application'
  return `${info.platform === 'linux' ? 'Linux' : info.platform} · ${info.architecture}`
})
const statusTitle = computed(() => {
  if (!updates.isDesktop.value) return 'Check from the desktop application'
  if (updates.checking.value) return 'Checking for updates…'
  if (updates.error.value) return updates.error.value.code === 'rate_limited' ? 'GitHub rate limit reached' : 'Could not check for updates'
  if (updates.result.value?.state === 'available') return `Version ${updates.result.value.latestVersion} is available`
  if (updates.result.value?.state === 'up_to_date') return 'You’re up to date'
  if (updates.result.value?.state === 'no_releases') return updates.info.value?.channel === 'stable' ? 'No stable releases yet' : 'No published releases yet'
  return 'Ready to check for updates'
})
const statusDescription = computed(() => {
  if (!updates.isDesktop.value) return 'Installed-version and release checks run in the native application. This browser preview makes no update requests.'
  if (updates.checking.value) return 'Looking for published Veyra releases on GitHub.'
  if (updates.error.value) return updates.error.value.message
  if (updates.result.value?.state === 'available') return 'Review the release notes and install the new Flatpak through your usual update workflow.'
  if (updates.result.value?.state === 'up_to_date') return updates.info.value?.channel === 'prerelease' ? 'No newer stable or pre-release version was found.' : 'No newer stable version was found.'
  if (updates.result.value?.state === 'no_releases') return updates.info.value?.channel === 'stable' ? 'No stable release is published for this update channel. Pre-release versions are excluded.' : 'A repository tag alone is not a downloadable release. Veyra will detect it once the release is published.'
  return 'Check now, or let Veyra check automatically once per day while it is open.'
})
onMounted(() => { void updates.initialize() })
</script>
<template>
  <div class="program-info">
    <div class="program-identity"><BrandMark/><div><h3>Veyra</h3><p class="muted">Your vault. One shortcut away.</p></div></div>
    <div class="program-version"><strong>Version {{ updates.version.value }}</strong><span class="status-chip">{{ channelLabel }}</span></div>
    <dl class="program-details">
      <div><dt>Platform</dt><dd>{{ platformLabel }}</dd></div>
      <div><dt>License</dt><dd>{{ updates.info.value?.license || 'GPL-3.0-only' }}</dd></div>
      <div><dt>Project</dt><dd>Lucas-Gruber/Veyra</dd></div>
    </dl>
    <p v-if="updates.infoError.value" class="program-info-error" role="alert">{{ updates.infoError.value }}</p>
    <section class="update-section" aria-labelledby="updates-heading">
      <h4 id="updates-heading">Software updates</h4>
      <div :class="['update-status', { 'update-available': updates.result.value?.state === 'available' && !updates.error.value, 'update-error': !!updates.error.value }]" role="status" aria-live="polite">
        <LoaderCircle v-if="updates.checking.value" :size="20" class="spin"/>
        <Download v-else-if="updates.result.value?.state === 'available' && !updates.error.value" :size="20"/>
        <CheckCircle2 v-else-if="updates.result.value?.state === 'up_to_date' && !updates.error.value" :size="20"/>
        <Info v-else :size="20"/>
        <div><strong>{{ statusTitle }}</strong><p>{{ statusDescription }}</p></div>
      </div>
      <div class="update-actions">
        <button class="secondary-button" :disabled="!updates.isDesktop.value || updates.checking.value" @click="updates.checkForUpdates"><RefreshCw :size="14" :class="{ spin: updates.checking.value }"/>Check for updates</button>
        <button v-if="updates.result.value?.state === 'available' && !updates.error.value" class="primary-button release-button" :disabled="updates.checking.value" @click="updates.openRelease">View release <ArrowUpRight :size="14"/></button>
      </div>
      <p v-if="updates.isDesktop.value" class="update-check-time">Last successful check: {{ lastSuccessfulCheck }}</p>
      <label class="automatic-updates"><input type="checkbox" aria-label="Automatically check for updates" aria-describedby="automatic-update-description" :checked="prefs.automaticUpdateChecks" :disabled="!updates.isDesktop.value" @change="updates.setAutomaticChecks(($event.target as HTMLInputElement).checked)"><span>Automatically check for updates<small id="automatic-update-description">When due at startup, then once per day while Veyra is open.</small></span></label>
      <p class="update-privacy">Checks contact GitHub’s release API using standard network and application metadata. No vault or account data is sent. Updates are installed through Flatpak or your software manager.</p>
    </section>
    <div class="setting-note program-boundary"><ShieldCheck :size="18"/><span>Read-only Vaultwarden integration. Independent security review is pending. Offline unlock, browser autofill and vault editing are upcoming milestones.</span></div>
  </div>
</template>
<style scoped>
.program-identity { display: flex; align-items: center; gap: 15px; }
.program-identity > :first-child { width: 40px; height: 45px; }
.program-identity h3 { margin: 0; }
.program-identity p { margin: 6px 0 0; }
.program-version { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin-top: 25px; font-size: 13px; }
.program-version strong { font-weight: 550; }
.program-details { margin: 22px 0 0; padding: 0; }
.program-details > div { display: flex; justify-content: space-between; gap: 16px; border-bottom: 1px solid var(--border); padding: 11px 0; font-size: 11px; }
.program-details dt { color: var(--muted); }
.program-details dd { margin: 0; text-align: right; overflow-wrap: anywhere; }
.update-section { margin-top: 28px; }
.update-section h4 { margin: 0 0 13px; }
.update-status { display: flex; align-items: flex-start; gap: 12px; padding: 15px; border: 1px solid var(--border-strong); background: var(--surface); border-radius: 9px; color: var(--muted); }
.update-status > svg { flex-shrink: 0; margin-top: 1px; }
.update-status strong { display: block; font-size: 12px; color: var(--text); font-weight: 550; line-height: 1.5; }
.update-status p { font-size: 10px; line-height: 1.75; margin: 7px 0 0; }
.update-available { background: var(--accent-bg); color: var(--accent); }
.update-error { border-color: #dc8b8b55; }
.update-actions { display: flex; flex-wrap: wrap; gap: 9px; margin-top: 13px; }
.update-actions button { min-height: 35px; font-size: 10px; }
.release-button { display: inline-flex; align-items: center; gap: 8px; padding: 8px 12px; border-radius: 7px; background: var(--accent); color: #19171f; font-weight: 550; }
.update-check-time { font-size: 9px; color: var(--subtle); margin: 12px 0 0; }
.automatic-updates { display: flex; align-items: flex-start; gap: 10px; font-size: 11px; margin-top: 24px; cursor: pointer; }
.automatic-updates input { margin-top: 2px; accent-color: var(--accent); width: 15px; height: 15px; flex-shrink: 0; }
.automatic-updates small { display: block; color: var(--muted); font-size: 9px; line-height: 1.7; margin-top: 6px; }
.update-privacy { font-size: 9px; color: var(--subtle); line-height: 1.8; margin: 12px 0 0 25px; }
.program-info-error { font-size: 11px; line-height: 1.7; color: #d78f8f; }
.program-boundary { font-size: 10px; }
@media (max-width: 560px) { .program-details > div { font-size: 10px; }.update-status { padding: 12px; }.update-status strong { font-size: 11px; }.program-version { font-size: 12px; }.update-actions button { flex: 1; justify-content: center; }.update-privacy { margin-left: 0; } }
</style>
