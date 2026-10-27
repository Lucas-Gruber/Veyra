import type { VaultItem } from '~/types/vault'

// Synthetic, non-sensitive display metadata. Credential values belong to Rust.
export const demoItems: readonly VaultItem[] = [
  { id: 'github', name: 'GitHub', username: 'alex@veyra.example', domain: 'github.com', url: 'https://github.com', kind: 'login', favorite: true, folder: 'Work', color: '#d2d6e5', initials: 'Gh', updated: 'Today', note: 'My home for things I build.\n\nPersonal account · development workspace', tags: ['Development', 'Personal'] },
  { id: 'linear', name: 'Linear', username: 'alex@veyra.example', domain: 'linear.app', url: 'https://linear.app', kind: 'login', favorite: true, folder: 'Work', color: '#8b80f9', initials: 'Li', updated: 'Today', note: 'A little more focus. A little less friction.', tags: ['Productivity', 'Work'] },
  { id: 'vercel', name: 'Vercel', username: 'alex@veyra.example', domain: 'vercel.com', url: 'https://vercel.com', kind: 'login', favorite: true, folder: 'Work', color: '#cdd1dd', initials: '▲', updated: 'Yesterday', tags: ['Development', 'Work'] },
  { id: 'proton', name: 'Proton Mail', username: 'alex@veyra.example', domain: 'proton.me', url: 'https://proton.me', kind: 'login', favorite: false, folder: 'Personal', color: '#bb91ff', initials: 'Pm', updated: 'Yesterday', tags: ['Email', 'Personal'] },
  { id: 'figma', name: 'Figma', username: 'alex@veyra.example', domain: 'figma.com', url: 'https://figma.com', kind: 'login', favorite: false, folder: 'Work', color: '#f297a3', initials: 'Fi', updated: 'Oct 22', tags: ['Design', 'Work'] },
  { id: 'tailscale', name: 'Tailscale', username: 'alex@veyra.example', domain: 'tailscale.com', url: 'https://tailscale.com', kind: 'login', favorite: false, folder: 'Work', color: '#80c6ae', initials: 'Ts', updated: 'Oct 20', tags: ['Infrastructure'] },
  { id: 'notion', name: 'Notion', username: 'alex@veyra.example', domain: 'notion.so', url: 'https://notion.so', kind: 'login', favorite: false, folder: 'Personal', color: '#d4c8b0', initials: 'N', updated: 'Oct 18', tags: ['Notes', 'Personal'] },
  { id: 'cloudflare', name: 'Cloudflare', username: 'alex@veyra.example', domain: 'cloudflare.com', url: 'https://cloudflare.com', kind: 'login', favorite: false, folder: 'Work', color: '#f6ad77', initials: 'Cf', updated: 'Oct 15', tags: ['Infrastructure', 'Work'] },
]
