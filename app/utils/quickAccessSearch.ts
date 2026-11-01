// Per-WebView memory only. Never serialize searches into preferences or storage.
interface QuickAccessSearch { query: string; selectedItemId: string | null }
let remembered: QuickAccessSearch & { generation: number; expiresAt: number; wallDeadline: number } | undefined
let expiry: ReturnType<typeof setTimeout> | undefined

export function clearQuickAccessSearch() {
  clearTimeout(expiry)
  expiry = undefined
  remembered = undefined
}

export function rememberQuickAccessSearch(query: string, generation: number, selectedItemId: string | null) {
  clearQuickAccessSearch()
  if (!query && !selectedItemId) return
  remembered = { query, selectedItemId, generation, expiresAt: performance.now() + 30_000, wallDeadline: Date.now() + 30_000 }
  expiry = setTimeout(clearQuickAccessSearch, 30_000)
}

export function restoreQuickAccessSearch(generation: number): QuickAccessSearch {
  const saved = remembered
  clearQuickAccessSearch()
  // Check the deadline even if a hidden WebView throttled its expiry timer.
  // Wall time also expires a search after suspend on engines whose monotonic
  // clock pauses during sleep; the monotonic deadline handles clock rollback.
  return saved && saved.generation === generation && performance.now() < saved.expiresAt && Date.now() < saved.wallDeadline
    ? { query: saved.query, selectedItemId: saved.selectedItemId }
    : { query: '', selectedItemId: null }
}
