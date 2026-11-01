import { describe, expect, it } from 'vitest'
import { defaultShortcuts, loadShortcuts, normalizeShortcut, shortcutFromEvent } from '../../app/utils/shortcuts'

describe('shortcut preferences', () => {
  it('normalizes modifier order and rejects invalid combinations', () => {
    expect(normalizeShortcut('Shift+Ctrl+c')).toBe('Ctrl+Shift+C')
    expect(normalizeShortcut('Alt+Ctrl+Space')).toBe('Ctrl+Alt+Space')
    for (const key of ['Ctrl+Ctrl+C', 'Ctrl+', 'Control+C', 'Ctrl+Unknown', '<script>']) expect(normalizeShortcut(key)).toBeNull()
  })
  it('matches the actual key and exact modifiers', () => {
    expect(shortcutFromEvent({ key: 'C', ctrlKey: true, shiftKey: true, altKey: false, metaKey: false })).toBe('Ctrl+Shift+C')
    expect(shortcutFromEvent({ key: ' ', ctrlKey: true, shiftKey: false, altKey: true, metaKey: false })).toBe('Ctrl+Alt+Space')
    expect(shortcutFromEvent({ key: 'Control', ctrlKey: true, shiftKey: false, altKey: false, metaKey: false })).toBeNull()
  })
  it('restores swapped bindings without creating conflicting actions', () => {
    const swapped = { ...defaultShortcuts, username: defaultShortcuts.password, password: defaultShortcuts.username }
    expect(loadShortcuts(swapped)).toEqual(swapped)
    expect(loadShortcuts({ username: defaultShortcuts.password })).toEqual(defaultShortcuts)
    expect(loadShortcuts({ lock: 'Ctrl+Q', password: null })).toMatchObject({ lock: 'Ctrl+Q', password: 'Ctrl+Shift+C' })
  })
})
