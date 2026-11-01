import { describe, expect, it } from 'vitest'
import { buildFolderTree, folderContainsItem } from '../../app/utils/folders'

describe('nested folders', () => {
  const folders = [{ id: 'mail', name: 'Work/Accounts/Mail' }, { id: 'empty', name: 'Work/Empty' }, { id: 'work', name: 'Work' }, { id: 'other', name: 'Workshop' }]
  it('builds virtual parents, preserves empty folders and rolls counts up once', () => {
    const tree = buildFolderTree(folders, [{ folderId: 'mail', folder: 'outdated name' }, { folderId: 'work' }, { folderId: 'mail', deleted: true }, { folder: '' }])
    expect(tree.map(node => [node.path, node.count])).toEqual([['Work', 2], ['Workshop', 0]])
    expect(tree[0]?.children.map(node => [node.path, node.count])).toEqual([['Work/Accounts', 1], ['Work/Empty', 0]])
    expect(tree[0]?.children[0]?.children[0]).toMatchObject({ name: 'Mail', path: 'Work/Accounts/Mail', parentPath: 'Work/Accounts', count: 1 })
  })
  it('includes descendants when selecting a parent without matching a similar prefix', () => {
    expect(folderContainsItem({ folderId: 'mail' }, 'Work', folders)).toBe(true)
    expect(folderContainsItem({ folderId: 'other' }, 'Work', folders)).toBe(false)
    expect(folderContainsItem({ folderId: 'mail', deleted: true }, 'Work', folders)).toBe(false)
    expect(folderContainsItem({ folder: '' }, 'Work', folders)).toBe(false)
  })
  it('supports demo paths, merges duplicate path folders and sorts siblings', () => {
    const tree = buildFolderTree([{ id: 'a', name: 'Personal' }, { id: 'b', name: 'Personal' }], [{ folderId: 'a' }, { folderId: 'b' }, { folder: 'Personal/Travel' }, { folder: 'Personal/Accounts' }])
    expect(tree).toHaveLength(1)
    expect(tree[0]?.count).toBe(4)
    expect(tree[0]?.children.map(node => node.name)).toEqual(['Accounts', 'Travel'])
  })
  it('distinguishes an explicitly unnamed folder from an unfiled item', () => {
    const unnamed = [{ id: 'unnamed', name: '' }]
    expect(buildFolderTree(unnamed, [{ folderId: 'unnamed' }, {}])[0]).toMatchObject({ name: 'Unnamed folder', count: 1 })
    expect(folderContainsItem({ folderId: 'unnamed' }, '', unnamed)).toBe(true)
    expect(folderContainsItem({}, '', unnamed)).toBe(false)
  })
})
