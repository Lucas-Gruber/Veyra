export type ItemKind = 'login' | 'note' | 'card' | 'identity'
export interface VaultItem {
  id: string
  name: string
  username: string
  domain: string
  url: string
  kind: ItemKind
  favorite: boolean
  folder: string
  folderId?: string | null
  color: string
  initials: string
  updated: string
  note?: string // Synthetic demonstration only; connected metadata never contains notes.
  tags: string[]
  hasPassword?: boolean
  hasTotp?: boolean
  readOnly?: boolean
  organizationId?: string | null
  collectionIds?: string[]
  revision?: string
  deleted?: boolean
  canDelete?: boolean
  canRestore?: boolean
}
export type CopyField = 'username' | 'password' | 'totp'
export interface SessionStatus {
  locked: boolean
  mode: 'connected' | 'demo' | 'disconnected'
  generation: number
  lastSync: string | null
}
export interface VaultFolder { id: string; name: string }
export interface VaultOrganization { id: string; name: string }
export interface VaultCollection { id: string; organizationId: string; name: string; readOnly: boolean; hidePasswords: boolean }
export interface VaultSnapshot { session: SessionStatus; items: VaultItem[]; unsupportedItems: number; organizations: VaultOrganization[]; collections: VaultCollection[]; folders: VaultFolder[]; needsSync: boolean }
export type CardField = 'cardholderName' | 'number' | 'code' | 'expMonth' | 'expYear' | 'brand'
export type IdentityField = 'title' | 'firstName' | 'middleName' | 'lastName' | 'address1' | 'address2' | 'address3' | 'city' | 'state' | 'postalCode' | 'country' | 'company' | 'email' | 'phone' | 'ssn' | 'username' | 'passportNumber' | 'licenseNumber'
export interface ItemDraft {
  id: string | null; revision: string | null; organizationId: string | null; collectionIds: string[]
  kind: ItemKind; name: string; favorite: boolean; notes: string; username: string; password: string; totp: string; uri: string
  card: Partial<Record<CardField, string | null>> | null
  identity: Partial<Record<IdentityField, string | null>> | null
}
export interface EditableItem { draft: ItemDraft; generation: number }
export interface WriteOutcome { itemId: string; refreshPending: boolean; snapshot: VaultSnapshot }
export type TwoFactorProvider = 'totp' | 'email'
export interface LoginRequest {
  serverUrl: string
  email: string
  masterPassword: string
  twoFactor?: { provider: TwoFactorProvider; code: string }
}
export type LoginOutcome = { status: 'authenticated'; session: SessionStatus } | { status: 'two_factor_required'; providers: TwoFactorProvider[]; session: SessionStatus }
export interface VaultCommandError { code: string; message: string }
export type ManagementOperation =
  | { type: 'assign_folder'; folderId: string | null }
  | { type: 'assign_collections'; collectionIds: string[] }
  | { type: 'copy' | 'move'; organizationId: string | null; collectionIds: string[]; folderId: string | null }
export interface ManagedItemResult { sourceId: string; itemId: string | null; status: 'succeeded' | 'failed' | 'uncertain' | 'not_attempted'; error: VaultCommandError | null }
export interface BatchOutcome { results: ManagedItemResult[]; refreshPending: boolean; snapshot: VaultSnapshot }
