<script setup lang="ts">
import { X, Palette, Command, ShieldCheck, Monitor, Moon, Sun, Info } from '@lucide/vue'
const emit = defineEmits<{ close: [] }>()
const prefs = usePreferences()
const vault = useVault()
const appUpdates = useAppUpdates()
const tab = ref('Appearance')
const panel = ref<HTMLElement>()
const initialFocus = ref<HTMLButtonElement>()
let previousFocus: HTMLElement | null = null
onMounted(() => { previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null; initialFocus.value?.focus() })
onBeforeUnmount(() => previousFocus?.focus())
function onKey(event: KeyboardEvent) {
  if (event.key === 'Escape') { event.stopPropagation(); emit('close') }
  if (event.key === 'Tab' && panel.value) {
    const nodes = Array.from(panel.value.querySelectorAll<HTMLElement>('button:not([disabled]), select:not([disabled]), a[href], input:not([disabled]), [tabindex="0"]:not([disabled])'))
    const first = nodes[0], last = nodes[nodes.length - 1]
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus() }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus() }
  }
}
</script>
<template>
  <div class="modal-backdrop" @click.self="emit('close')">
    <section ref="panel" class="settings-dialog" role="dialog" aria-modal="true" aria-labelledby="settings-heading" @keydown="onKey">
      <div class="settings-sidebar"><h2 id="settings-heading">Settings</h2><p>Make yourself at home.</p>
        <button v-for="entry in [{ label: 'Appearance', icon: Palette }, { label: 'Quick Access', icon: Command }, { label: 'Security', icon: ShieldCheck }, { label: 'About', icon: Info }]" :key="entry.label" :class="{ active: tab === entry.label }" @click="tab = entry.label"><component :is="entry.icon" :size="17"/>{{ entry.label }}</button>
        <div class="settings-version"><BrandMark/> Veyra <span>{{ appUpdates.version.value }}</span></div>
      </div>
      <div class="settings-body"><button ref="initialFocus" class="icon-button settings-close" aria-label="Close settings" @click="emit('close')"><X :size="19"/></button>
        <template v-if="tab === 'Appearance'">
          <p class="eyebrow">YOUR SPACE, YOUR WAY</p><h3>Appearance</h3><p class="muted">A familiar feeling, with a little personality.</p>
          <h4>Color theme</h4><div class="theme-options">
            <button v-for="theme in [{ value: 'light' as const, icon: Sun, name: 'Light' }, { value: 'dark' as const, icon: Moon, name: 'Dark' }, { value: 'system' as const, icon: Monitor, name: 'System' }]" :key="theme.value" :class="['theme-option', theme.value, { selected: prefs.appearance === theme.value }]" :aria-pressed="prefs.appearance === theme.value" @click="prefs.appearance = theme.value; prefs.persist()"><div class="theme-preview"><span/><span/><span/></div><span><component :is="theme.icon" :size="15"/>{{ theme.name }}</span></button>
          </div>
          <h4>Surface</h4><div class="segmented-control"><button v-for="surface in ['modern', 'glass'] as const" :key="surface" :class="{ active: prefs.surface === surface }" @click="prefs.surface = surface; prefs.persist()">{{ surface === 'modern' ? 'Modern' : 'Glass' }}</button></div><p class="setting-hint">Glass uses subtle in-window translucency. Desktop blur depends on your compositor.</p>
          <div class="setting-note"><Palette :size="18"/><span>Quietly violet. Veyra’s accent keeps focus where it belongs.</span></div>
        </template>
        <template v-else-if="tab === 'Quick Access'">
          <p class="eyebrow">LESS REACHING, MORE DOING</p><h3>One shortcut away.</h3><p class="muted">Find an item. Take an action. Get back to your day.</p>
          <div class="setting-row"><span>Open Quick Access<small>Within Veyra in this milestone</small></span><kbd>Ctrl</kbd><kbd>Alt</kbd><kbd>Space</kbd></div>
          <label class="setting-row"><span>Default action<small>When pressing Enter on a result</small></span><select v-model="prefs.defaultAction" @change="prefs.persist()"><option value="password">Copy password</option><option value="username">Copy username</option><option value="website">Open website</option></select></label>
          <div class="shortcut-table"><span>Copy username</span><kbd>Ctrl U</kbd><span>Copy password</span><kbd>Ctrl P</kbd><span>Copy one-time code</span><kbd>Ctrl O</kbd><span>Open website</span><kbd>Ctrl B</kbd><span>Show actions</span><kbd>Tab</kbd><span>Close</span><kbd>Esc</kbd></div>
          <p class="setting-hint">System-wide shortcuts and shortcut customization require portal integration in a later milestone. Ctrl U keeps its text editing behavior in the search field; copy username from the action menu instead.</p>
        </template>
        <template v-else-if="tab === 'Security'">
          <p class="eyebrow">CLEAR BOUNDARIES</p><h3>Built with care.</h3><p class="muted">{{ vault.connected.value ? 'Connected vault access is read-only and held in memory.' : 'Explore samples, or connect securely in the desktop application.' }}</p>
          <div class="setting-row"><span>Clipboard clearing<small>Desktop · best effort, only if unchanged</small></span><span class="status-chip">30 seconds</span></div>
          <div class="setting-row"><span>Vault data<small>{{ vault.demo.value ? 'Synthetic metadata, held in memory' : 'Decrypted metadata in memory while unlocked' }}</small></span><ShieldCheck :size="19"/></div>
          <div class="setting-note"><Info :size="20"/><span>{{ vault.desktop.value ? 'Clipboard history managers may retain copied data. Veyra cannot guarantee erasure outside the application.' : 'Browser demo: clipboard clearing is unavailable. Only synthetic sample values can be copied.' }}</span></div>
          <p class="setting-hint">This is an integration preview pending independent security review. Locking signs out and drops in-memory keys. Automatic locking, offline cache and write operations are not available yet.</p>
        </template>
        <AboutPanel v-else/>
      </div>
    </section>
  </div>
</template>
