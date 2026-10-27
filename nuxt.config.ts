import packageInfo from './package.json' with { type: 'json' }

export default defineNuxtConfig({
  compatibilityDate: '2026-10-26',
  ssr: false,
  devtools: { enabled: false },
  modules: ['@nuxt/ui', '@pinia/nuxt', '@nuxt/eslint'],
  css: ['~/assets/css/main.css'],
  typescript: { strict: true },
  appConfig: { appVersion: packageInfo.version },
  ui: { fonts: false },
  app: {
    head: {
      title: 'Veyra — Your vault. One shortcut away.',
      meta: [{ name: 'description', content: 'A Linux-first Vaultwarden companion with read-only personal vault access.' }],
      link: [{ rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' }],
    },
  },
  nitro: { preset: 'static' },
  telemetry: false,
  vite: { optimizeDeps: { include: ['@lucide/vue', '@tauri-apps/api/core', '@tauri-apps/api/event'] } },
})
