/* SPDX-License-Identifier: GPL-3.0-only */
(() => {
  'use strict'
  if (window.top !== window || location.protocol !== 'https:') return
  const api = globalThis.browser || globalThis.chrome
  const handled = new Set()

  async function requestFill() {
    const target = globalThis.VeyraFill.parseTarget(location.href)
    if (!target || handled.has(target.intentId)) return
    handled.add(target.intentId)
    let credentials
    let filled = false
    try {
      // Wait for a suitable form before requesting any plaintext credentials.
      let ready = false
      for (let attempt = 0; attempt < 20; attempt++) {
        const current = globalThis.VeyraFill.parseTarget(location.href)
        if (!current || current.intentId !== target.intentId || current.targetUrl !== target.targetUrl) return
        ready = globalThis.VeyraFill.canFill(location.href)
        if (ready) break
        await new Promise(resolve => setTimeout(resolve, 100))
      }
      if (!ready) {
        history.replaceState(history.state, '', target.targetUrl)
        return
      }
      credentials = await api.runtime.sendMessage({ type: 'veyra_take', ...target })
      // Navigation, route changes or a superseded request cancel filling.
      const current = globalThis.VeyraFill.parseTarget(location.href)
      if (!current || current.intentId !== target.intentId || current.targetUrl !== target.targetUrl) return
      history.replaceState(history.state, '', target.targetUrl)
      if (credentials?.status === 'credentials') {
        filled = globalThis.VeyraFill.fillLogin(credentials, target.targetUrl)
      }
    } catch {
      // No credentials, server bodies or page values are logged or persisted.
    } finally {
      // eslint-disable-next-line no-useless-assignment -- Release our transient reference; JavaScript cannot guarantee erasure.
      credentials = undefined
      try { await api.runtime.sendMessage({ type: 'veyra_ack', ...target, filled }) } catch { /* desktop timeout reports failure */ }
    }
  }

  void api.runtime.sendMessage({ type: 'veyra_hello' }).catch(() => {})
  void requestFill()
  addEventListener('hashchange', () => { void requestFill() })
})()
