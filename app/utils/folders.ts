export interface FolderSource { id: string; name: string }
export interface FolderItemSource { folderId?: string | null; folder?: string; deleted?: boolean }
export interface FolderNode {
  path: string
  name: string
  parentPath: string | null
  count: number
  children: FolderNode[]
}

function itemPath(item: FolderItemSource, names: ReadonlyMap<string, string>): string | null {
  if (item.folderId && names.has(item.folderId)) return names.get(item.folderId)!
  return item.folder || null
}

/** Selecting a parent includes its descendants, without matching similar names. */
export function folderContainsItem(item: FolderItemSource, path: string, folders: readonly FolderSource[]): boolean {
  if (item.deleted) return false
  const actual = itemPath(item, new Map(folders.map(folder => [folder.id, folder.name])))
  return actual !== null && (actual === path || actual.startsWith(`${path}/`))
}

/** Keep actual empty folders and synthesize missing parents from slash paths. */
export function buildFolderTree(folders: readonly FolderSource[], items: readonly FolderItemSource[]): FolderNode[] {
  const roots: FolderNode[] = []
  const nodes = new Map<string, FolderNode>()
  const names = new Map(folders.map(folder => [folder.id, folder.name]))
  function ensurePath(path: string): FolderNode {
    const segments = path.split('/')
    let parent: FolderNode | undefined
    for (let index = 0; index < segments.length; index++) {
      const key = segments.slice(0, index + 1).join('/')
      let node = nodes.get(key)
      if (!node) {
        node = { path: key, name: segments[index] || 'Unnamed folder', parentPath: parent?.path ?? null, count: 0, children: [] }
        nodes.set(key, node)
        if (parent) parent.children.push(node)
        else roots.push(node)
      }
      parent = node
    }
    return parent!
  }
  for (const folder of folders) ensurePath(folder.name)
  for (const item of items) {
    if (item.deleted) continue
    const path = itemPath(item, names)
    if (path === null) continue
    let node: FolderNode | undefined = ensurePath(path)
    while (node) {
      node.count++
      node = node.parentPath === null ? undefined : nodes.get(node.parentPath)
    }
  }
  function sort(nodes: FolderNode[]) {
    nodes.sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: 'base', numeric: true }) || a.path.localeCompare(b.path))
    for (const node of nodes) sort(node.children)
  }
  sort(roots)
  return roots
}
