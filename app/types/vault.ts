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
  color: string
  initials: string
  updated: string
  note?: string // Synthetic demonstration only; connected metadata never contains notes.
  tags: string[]
  hasPassword?: boolean
  hasTotp?: boolean
  readOnly?: boolean
}
export type CopyField = 'username' | 'password' | 'totp'
export interface SessionStatus {
  locked: boolean
  mode: 'connected' | 'demo' | 'disconnected'
  generation: number
  lastSync: string | null
}
export interface VaultSnapshot { session: SessionStatus; items: VaultItem[]; unsupportedItems: number }
export type TwoFactorProvider = 'totp' | 'email'
export interface LoginRequest {
  serverUrl: string
  email: string
  masterPassword: string
  twoFactor?: { provider: TwoFactorProvider; code: string }
}
export type LoginOutcome = { status: 'authenticated'; session: SessionStatus } | { status: 'two_factor_required'; providers: TwoFactorProvider[]; session: SessionStatus }
export interface VaultCommandError { code: string; message: string }
