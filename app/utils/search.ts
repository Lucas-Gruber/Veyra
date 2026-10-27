import type { VaultItem } from '~/types/vault'

function fuzzyScore(value: string, query: string): number {
  const text = value.toLocaleLowerCase()
  if (text === query) return 120
  if (text.startsWith(query)) return 100
  const position = text.indexOf(query)
  if (position >= 0) return 80 - Math.min(position, 30)
  let cursor = 0
  let spread = 0
  for (const char of query) {
    const index = text.indexOf(char, cursor)
    if (index < 0) return 0
    spread += index - cursor
    cursor = index + 1
  }
  return Math.max(1, 40 - spread)
}

export function searchItems(items: readonly VaultItem[], query: string): VaultItem[] {
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean)
  if (!terms.length) return [...items].sort((a, b) => Number(b.favorite) - Number(a.favorite) || a.name.localeCompare(b.name))
  return items.map(item => {
    const scores = terms.map(term => Math.max(
      fuzzyScore(item.name, term) * 1.5,
      fuzzyScore(item.domain, term) * 1.3,
      fuzzyScore(item.username, term),
      ...item.tags.map(tag => fuzzyScore(tag, term)),
    ))
    return { item, score: scores.every(score => score > 0) ? scores.reduce((sum, score) => sum + score, 0) + (item.favorite ? 5 : 0) : 0 }
  }).filter(result => result.score > 0).sort((a, b) => b.score - a.score || a.item.name.localeCompare(b.item.name)).map(result => result.item)
}

export function highlightSegments(value: string, query: string): { text: string; match: boolean }[] {
  const term = query.trim().toLocaleLowerCase()
  const index = term ? value.toLocaleLowerCase().indexOf(term) : -1
  if (index < 0) return [{ text: value, match: false }]
  return [
    { text: value.slice(0, index), match: false },
    { text: value.slice(index, index + term.length), match: true },
    { text: value.slice(index + term.length), match: false },
  ].filter(segment => segment.text.length > 0)
}
