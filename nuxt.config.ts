export default defineNuxtConfig({
  compatibilityDate: '2026-10-26',
  ssr: false,
  devtools: { enabled: false },
  modules: ['@nuxt/ui', '@pinia/nuxt', '@nuxt/eslint'],
  css: ['~/assets/css/main.css'],
  typescript: { strict: true },
  ui: { fonts: false },
  app: {
    head: {
      title: 'Veyra — Your vault. One shortcut away.',
      meta: [{ name: 'description', content: 'A thoughtfully crafted Linux vault companion. First milestone: synthetic demo only.' }],
      link: [{ rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' }],
    },
  },
  nitro: { preset: 'static' },
  telemetry: false,
  vite: { optimizeDeps: { include: ['@lucide/vue', '@tauri-apps/api/core', '@tauri-apps/api/event'] } },
})
