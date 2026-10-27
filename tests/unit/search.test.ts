import { describe, expect, it } from 'vitest'
import { highlightSegments, searchItems } from '../../app/utils/search'
import type { VaultItem } from '../../app/types/vault'
const item = (id: string, name: string, domain: string, favorite = false): VaultItem => ({
  id, name, domain, favorite, url: `https://${domain}`, username: `${id}@example.test`,
  kind: 'login', folder: 'Personal', color: '#fff', initials: name.slice(0, 2), updated: 'Today', tags: [],
})
const items = [item('github', 'GitHub', 'github.com'), item('gitlab', 'GitLab', 'gitlab.com'), item('google', 'Google', 'accounts.google.com', true)]
describe('local vault metadata search', () => {
  it('ranks an exact name match first', () => expect(searchItems(items, 'github').map(x => x.id)).toEqual(['github']))
  it('finds a domain without relying on the display name', () => expect(searchItems(items, 'accounts.google.com')[0]?.id).toBe('google'))
  it('supports fuzzy keyboard queries', () => expect(searchItems(items, 'gthb')[0]?.id).toBe('github'))
  it('requires all query terms', () => expect(searchItems(items, 'GitHub nonexistent')).toEqual([]))
  it('handles whitespace and case', () => expect(searchItems(items, '  GITHUB  ')[0]?.id).toBe('github'))
  it('ranks favorites in an empty search without mutating source', () => {
    const before = items.map(x => x.id)
    expect(searchItems(items, ' ')[0]?.id).toBe('google')
    expect(items.map(x => x.id)).toEqual(before)
  })
  it('does not create results when no metadata is available', () => expect(searchItems([], 'github')).toEqual([]))
  it('keeps highlighting as text, including malicious-looking metadata', () => {
    const value = '<script>alert(1)</script>'
    const segments = highlightSegments(value, 'script')
    expect(segments.map(x => x.text).join('')).toBe(value)
    expect(segments.some(x => x.match && x.text === 'script')).toBe(true)
  })
})
