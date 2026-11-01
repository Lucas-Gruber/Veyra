export const shortcutDefinitions = [
  { id: 'openQuick', label: 'Open Quick Access', key: 'Ctrl+Alt+Space' },
  { id: 'search', label: 'Search vault', key: 'Ctrl+K' },
  { id: 'lock', label: 'Lock vault', key: 'Ctrl+L' },
  { id: 'settings', label: 'Open settings', key: 'Ctrl+,' },
  { id: 'username', label: 'Copy username', key: 'Ctrl+C' },
  { id: 'password', label: 'Copy password', key: 'Ctrl+Shift+C' },
  { id: 'totp', label: 'Copy one-time code', key: 'Ctrl+Alt+C' },
  { id: 'autofill', label: 'Open website and fill credentials', key: 'Enter' },
  { id: 'website', label: 'Open website', key: 'Ctrl+B' },
  { id: 'detail', label: 'Open Veyra', key: 'Ctrl+E' },
  { id: 'favorite', label: 'Toggle favorite', key: 'Ctrl+D' },
  { id: 'quickActions', label: 'Quick Access actions', key: 'Tab' },
  { id: 'quickClose', label: 'Close Quick Access', key: 'Escape' },
  { id: 'quickNext', label: 'Next Quick Access result', key: 'ArrowDown' },
  { id: 'quickPrevious', label: 'Previous Quick Access result', key: 'ArrowUp' },
] as const
export type ShortcutAction = typeof shortcutDefinitions[number]['id']
export type ShortcutMap = Record<ShortcutAction, string>
export const defaultShortcuts = Object.fromEntries(shortcutDefinitions.map(entry => [entry.id, entry.key])) as ShortcutMap
const namedKeys = new Set(['Space', 'Enter', 'Escape', 'Tab', 'ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Home', 'End', 'Delete', 'Backspace', 'Insert', 'PageUp', 'PageDown'])
const modifierKeys = new Set(['Control', 'Alt', 'Shift', 'Meta', 'AltGraph', 'CapsLock', 'Unidentified', 'Dead'])
export function normalizeShortcut(value: unknown): string | null {
  if (typeof value !== 'string' || value.length > 80) return null
  const parts = value.split('+')
  const key = parts.pop()
  if (!key || (!namedKeys.has(key) && !/^[A-Za-z0-9,.;/[\]\\'`=-]$/.test(key) && !/^F(?:[1-9]|1[0-9]|2[0-4])$/.test(key))) return null
  if (new Set(parts).size !== parts.length || parts.some(part => !['Ctrl', 'Alt', 'Shift', 'Meta'].includes(part))) return null
  return [...['Ctrl', 'Alt', 'Shift', 'Meta'].filter(modifier => parts.includes(modifier)), key.length === 1 ? key.toUpperCase() : key].join('+')
}
export function shortcutFromEvent(event: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey'>): string | null {
  if (modifierKeys.has(event.key)) return null
  return normalizeShortcut([...event.ctrlKey ? ['Ctrl'] : [], ...event.altKey ? ['Alt'] : [], ...event.shiftKey ? ['Shift'] : [], ...event.metaKey ? ['Meta'] : [], event.key === ' ' ? 'Space' : event.key].join('+'))
}
export function matchesShortcut(event: KeyboardEvent, shortcuts: ShortcutMap, action: ShortcutAction) { return shortcutFromEvent(event) === shortcuts[action] }
export function loadShortcuts(value: unknown): ShortcutMap {
  const result = { ...defaultShortcuts }
  if (!value || typeof value !== 'object') return result
  for (const entry of shortcutDefinitions) {
    const key = normalizeShortcut((value as Record<string, unknown>)[entry.id])
    if (key) result[entry.id] = key
  }
  // Validate the final map so restoring swapped bindings works as well.
  return new Set(Object.values(result)).size === shortcutDefinitions.length ? result : { ...defaultShortcuts }
}
export function hasSelectedInputText(target: EventTarget | null): boolean {
  return (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) && target.selectionStart !== target.selectionEnd
}
