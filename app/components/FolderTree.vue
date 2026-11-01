<script setup lang="ts">
import { ChevronRight, FolderClosed, FolderOpen } from '@lucide/vue'
import type { ComponentPublicInstance } from 'vue'
import { buildFolderTree, type FolderItemSource, type FolderNode, type FolderSource } from '~/utils/folders'

const props = defineProps<{ folders: readonly FolderSource[]; items: readonly FolderItemSource[]; selectedPath?: string | null }>()
const emit = defineEmits<{ select: [path: string] }>()
const collapsed = ref(new Set<string>())
const focusedPath = ref<string | null>(null)
const rows = new Map<string, HTMLElement>()
const tree = computed(() => buildFolderTree(props.folders, props.items))
interface VisibleFolder { node: FolderNode; level: number; position: number; siblings: number }
const visible = computed(() => {
  const entries: VisibleFolder[] = []
  function visit(nodes: FolderNode[], level: number) {
    nodes.forEach((node, index) => {
      entries.push({ node, level, position: index + 1, siblings: nodes.length })
      if (!collapsed.value.has(node.path)) visit(node.children, level + 1)
    })
  }
  visit(tree.value, 1)
  return entries
})
watch(visible, entries => {
  if (!entries.some(entry => entry.node.path === focusedPath.value)) focusedPath.value = entries[0]?.node.path ?? null
}, { immediate: true })
watch(() => props.selectedPath, path => {
  if (path == null) return
  const segments = path.split('/')
  for (let index = 1; index < segments.length; index++) collapsed.value.delete(segments.slice(0, index).join('/'))
}, { immediate: true })
function registerRow(path: string, element: Element | ComponentPublicInstance | null) {
  if (element instanceof HTMLElement) rows.set(path, element)
  else rows.delete(path)
}
function toggle(path: string) {
  if (collapsed.value.has(path)) collapsed.value.delete(path)
  else collapsed.value.add(path)
}
function focus(path: string | undefined) {
  if (path === undefined) return
  focusedPath.value = path
  rows.get(path)?.focus()
}
function select(path: string) { focusedPath.value = path; emit('select', path) }
function onKey(event: KeyboardEvent, entry: VisibleFolder, index: number) {
  if (!['ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Home', 'End', 'Enter', ' '].includes(event.key)) return
  event.preventDefault()
  event.stopPropagation()
  const { node } = entry
  if (event.key === 'ArrowDown') focus(visible.value[index + 1]?.node.path)
  else if (event.key === 'ArrowUp') focus(visible.value[index - 1]?.node.path)
  else if (event.key === 'Home') focus(visible.value[0]?.node.path)
  else if (event.key === 'End') focus(visible.value.at(-1)?.node.path)
  else if (event.key === 'ArrowRight' && node.children.length) {
    if (collapsed.value.has(node.path)) collapsed.value.delete(node.path)
    else focus(node.children[0]?.path)
  } else if (event.key === 'ArrowLeft') {
    if (node.children.length && !collapsed.value.has(node.path)) collapsed.value.add(node.path)
    else if (node.parentPath !== null) focus(node.parentPath)
  } else if (event.key === 'Enter' || event.key === ' ') select(node.path)
}
</script>

<template>
  <ul v-if="visible.length" class="folder-tree" role="tree" aria-label="Folders">
    <li v-for="(entry, index) in visible" :key="entry.node.path" :ref="(element: Element | ComponentPublicInstance | null) => registerRow(entry.node.path, element)" role="treeitem" :aria-level="entry.level" :aria-posinset="entry.position" :aria-setsize="entry.siblings" :aria-expanded="entry.node.children.length ? !collapsed.has(entry.node.path) : undefined" :aria-selected="selectedPath === entry.node.path" :tabindex="focusedPath === entry.node.path ? 0 : -1" :class="['folder-row', { active: selectedPath === entry.node.path }]" :style="{ '--folder-depth': Math.min(entry.level - 1, 6) }" :title="entry.node.path || 'Unnamed folder'" @focus="focusedPath = entry.node.path" @click="select(entry.node.path)" @keydown="onKey($event, entry, index)">
      <button v-if="entry.node.children.length" class="folder-toggle" tabindex="-1" :aria-label="`${collapsed.has(entry.node.path) ? 'Expand' : 'Collapse'} ${entry.node.name}`" @click.stop="toggle(entry.node.path)"><ChevronRight :size="14" :class="{ expanded: !collapsed.has(entry.node.path) }"/></button>
      <span v-else class="folder-toggle-spacer"/>
      <component :is="entry.node.children.length && !collapsed.has(entry.node.path) ? FolderOpen : FolderClosed" :size="17" class="folder-icon" aria-hidden="true"/>
      <span class="folder-name">{{ entry.node.name }}</span><span class="folder-count" :aria-label="`${entry.node.count} items`">{{ entry.node.count }}</span>
    </li>
  </ul>
  <p v-else class="folder-empty">No folders yet.</p>
</template>

<style scoped>
.folder-tree { list-style: none; margin: 0; padding: 0; }
.folder-row { display: flex; align-items: center; gap: 7px; min-height: 38px; margin: 1px 0; padding: 7px 10px 7px calc(2px + var(--folder-depth) * 15px); color: var(--muted); border-radius: 7px; cursor: pointer; font-size: .875rem; }
.folder-row:hover { color: var(--text); background: var(--surface); }
.folder-row.active { color: var(--accent); background: var(--accent-bg); }
.folder-row:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
.folder-toggle,.folder-toggle-spacer { width: 21px; height: 24px; flex-shrink: 0; }
.folder-toggle { display: grid; place-items: center; padding: 0; color: inherit; background: transparent; border-radius: 4px; }
.folder-toggle:hover { background: var(--surface-selected); }
.folder-toggle svg { transition: transform .15s; }
.folder-toggle svg.expanded { transform: rotate(90deg); }
.folder-icon { flex-shrink: 0; }
.folder-name { flex: 1; min-width: 0; overflow-wrap: anywhere; line-height: 1.4; }
.folder-count { margin-left: auto; font-size: .75rem; flex-shrink: 0; color: var(--subtle); }
.folder-empty { font-size: .8125rem; color: var(--subtle); padding: 8px 12px; }
@media (prefers-reduced-motion: reduce) { .folder-toggle svg { transition: none; } }
</style>
