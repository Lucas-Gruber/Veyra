<script setup lang="ts">
import { ChevronDown, Check, Building2, Layers, UserRound } from '@lucide/vue'
const scope = defineModel<string>({ required: true })
const vault = useVault()
const menu = ref<HTMLElement>()
const open = ref(false)
const choices = computed(() => [
  { id: 'all', name: vault.demo.value ? 'Demo vault' : 'All vaults', icon: Layers },
  { id: 'personal', name: 'Your vault', icon: UserRound },
  ...vault.organizations.value.map(org => ({ ...org, icon: Building2 })),
])
const current = computed(() => choices.value.find(choice => choice.id === scope.value) || choices.value[0]!)
function choose(id: string) { scope.value = id; open.value = false }
function outside(event: PointerEvent) { if (event.target instanceof Node && !menu.value?.contains(event.target)) open.value = false }
function focusOut(event: FocusEvent) { if (!(event.relatedTarget instanceof Node) || !menu.value?.contains(event.relatedTarget)) open.value = false }
function keyboard(event: KeyboardEvent) {
  if (event.key === 'Escape') { event.stopPropagation(); open.value = false; menu.value?.querySelector<HTMLButtonElement>('button')?.focus() }
  if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
    event.preventDefault(); open.value = true
    nextTick(() => {
      const buttons = Array.from(menu.value?.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]') || [])
      const index = buttons.findIndex(button => button === document.activeElement)
      buttons[event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 : (index + (event.key === 'ArrowUp' ? -1 : 1) + buttons.length) % buttons.length]?.focus()
    })
  }
}
watch(vault.locked, () => { open.value = false; scope.value = 'all' })
watch(choices, value => { if (!value.some(choice => choice.id === scope.value)) scope.value = 'all' })
onMounted(() => document.addEventListener('pointerdown', outside))
onBeforeUnmount(() => document.removeEventListener('pointerdown', outside))
</script>
<template>
  <div ref="menu" class="vault-switcher" @keydown="keyboard" @focusout="focusOut">
    <button class="account-switcher" :disabled="vault.locked.value" aria-label="Choose vault" aria-haspopup="menu" :aria-expanded="open" @click="open = !open"><span class="account-avatar"><component :is="current.icon" :size="18"/></span><span class="account-label"><strong>{{ current.name }}</strong><small><span class="status-dot"/>{{ vault.connected.value ? `${vault.organizations.value.length} organizations` : vault.demo.value ? 'Local demonstration' : 'Not connected' }}</small></span><ChevronDown :size="14"/></button>
    <div v-if="open" class="vault-menu" role="menu" aria-label="Vaults"><button v-for="choice in choices" :key="choice.id" role="menuitemradio" :aria-checked="scope === choice.id" @click="choose(choice.id)"><component :is="choice.icon" :size="17"/><span>{{ choice.name }}</span><Check v-if="scope === choice.id" :size="16"/></button></div>
  </div>
</template>
