import { defineStore } from 'pinia'

export type Appearance = 'system' | 'dark' | 'light'
export type Surface = 'modern' | 'glass'
export const usePreferences = defineStore('preferences', () => {
  const appearance = ref<Appearance>('dark')
  const surface = ref<Surface>('modern')
  const clipboardTimeout = ref(30)
  const defaultAction = ref<'password' | 'username' | 'website'>('password')
  function load() {
    try {
      const value = JSON.parse(localStorage.getItem('veyra:preferences') || '{}') as Record<string, unknown>
      if (['system', 'dark', 'light'].includes(String(value.appearance))) appearance.value = value.appearance as Appearance
      if (['modern', 'glass'].includes(String(value.surface))) surface.value = value.surface as Surface
      if (['password', 'username', 'website'].includes(String(value.defaultAction))) defaultAction.value = value.defaultAction as typeof defaultAction.value
    } catch { /* Invalid preferences fall back to safe defaults. */ }
  }
  function persist() {
    localStorage.setItem('veyra:preferences', JSON.stringify({ appearance: appearance.value, surface: surface.value, defaultAction: defaultAction.value }))
  }
  return { appearance, surface, clipboardTimeout, defaultAction, load, persist }
})
