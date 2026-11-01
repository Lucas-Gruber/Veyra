<script setup lang="ts">
export interface ContextAction { id: string; label: string; disabled?: boolean; danger?: boolean }
const props = defineProps<{ x: number; y: number; actions: ContextAction[] }>()
const emit = defineEmits<{ close: []; select: [id: string] }>()
const menu = ref<HTMLElement>()
const position = reactive({ left: props.x, top: props.y })
let previousFocus: HTMLElement | null = null
function buttons() { return Array.from(menu.value?.querySelectorAll<HTMLButtonElement>('button:not([disabled])') || []) }
function outside(event: PointerEvent) { if (event.target instanceof Node && !menu.value?.contains(event.target)) emit('close') }
function onKey(event: KeyboardEvent) {
  if (['Escape', 'Tab'].includes(event.key)) { event.preventDefault(); event.stopPropagation(); emit('close'); return }
  if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return
  event.preventDefault(); event.stopPropagation()
  const entries = buttons(), index = entries.indexOf(document.activeElement as HTMLButtonElement)
  const target = event.key === 'Home' ? 0 : event.key === 'End' ? entries.length - 1 : (index + (event.key === 'ArrowDown' ? 1 : -1) + entries.length) % entries.length
  entries[target]?.focus()
}
onMounted(async () => {
  previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
  await nextTick()
  const bounds = menu.value?.getBoundingClientRect()
  if (bounds) {
    position.left = Math.max(8, Math.min(props.x, window.innerWidth - bounds.width - 8))
    position.top = Math.max(8, Math.min(props.y, window.innerHeight - bounds.height - 8))
  }
  buttons()[0]?.focus()
  document.addEventListener('pointerdown', outside, true)
  window.addEventListener('resize', close)
  window.addEventListener('blur', close)
})
function close() { emit('close') }
onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', outside, true)
  window.removeEventListener('resize', close)
  window.removeEventListener('blur', close)
  if (previousFocus?.isConnected) previousFocus.focus()
})
</script>
<template>
  <div ref="menu" class="item-context-menu" role="menu" aria-label="Item actions" :style="{ left: `${position.left}px`, top: `${position.top}px` }" @keydown="onKey" @contextmenu.prevent>
    <button v-for="action in actions" :key="action.id" role="menuitem" :disabled="action.disabled" :class="{ danger: action.danger }" @click="emit('select', action.id)">{{ action.label }}</button>
  </div>
</template>
<style scoped>
.item-context-menu { position: fixed; z-index: 150; min-width: 220px; max-width: calc(100vw - 16px); max-height: calc(100dvh - 16px); overflow: auto; padding: 6px; border: 1px solid var(--border-strong); border-radius: 10px; background: var(--panel); box-shadow: 0 12px 40px #0005; }
button { display: block; width: 100%; padding: 10px 12px; border-radius: 5px; text-align: left; color: var(--text); font-size: .875rem; }
button:hover,button:focus-visible { background: var(--surface-selected); outline: none; }
button:disabled { opacity: .45; cursor: default; }
.danger { color: var(--danger, #ed9292); }
</style>
