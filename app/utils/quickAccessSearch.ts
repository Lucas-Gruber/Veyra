// Per-WebView memory only. Never serialize searches into preferences or storage.
let remembered: { query: string; generation: number; expiresAt: number } | undefined
let expiry: ReturnType<typeof setTimeout> | undefined

export function clearQuickAccessSearch() {
  clearTimeout(expiry)
  expiry = undefined
  remembered = undefined
}

export function rememberQuickAccessSearch(query: string, generation: number) {
  clearQuickAccessSearch()
  if (!query) return
  remembered = { query, generation, expiresAt: performance.now() + 30_000 }
  expiry = setTimeout(clearQuickAccessSearch, 30_000)
}

export function restoreQuickAccessSearch(generation: number): string {
  const saved = remembered
  clearQuickAccessSearch()
  // Check the deadline even if a hidden WebView throttled its expiry timer.
  return saved && saved.generation === generation && performance.now() < saved.expiresAt ? saved.query : ''
}
