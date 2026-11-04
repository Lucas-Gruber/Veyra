import { invoke } from '@tauri-apps/api/core'

export interface LoginPreferences { serverUrl: string; email: string }

// Only these two explicitly remembered fields cross this persistence boundary.
// Passwords and verification codes belong exclusively to the sign-in request.
export function useLoginPreferences() {
  return {
    load: () => invoke<LoginPreferences | null>('load_login_preferences'),
    save: ({ serverUrl, email }: LoginPreferences) => invoke<LoginPreferences>('save_login_preferences', { preferences: { serverUrl, email } }),
    clear: async () => { await invoke('clear_login_preferences') },
  }
}
