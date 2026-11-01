/* SPDX-License-Identifier: GPL-3.0-only */
(() => {
  'use strict'
  const api = globalThis.browser || globalThis.chrome
  const host = 'app.veyra.companion'
  const marker = /(?:^#|&)__veyra_fill=([0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12})$/

  function verifiedTarget(sender, intentId, targetUrl) {
    if (sender.id !== api.runtime.id || !sender.tab || sender.frameId !== 0 || typeof sender.url !== 'string') return null
    try {
      const url = new URL(sender.url)
      if (url.protocol !== 'https:' || url.username || url.password) return null
      const match = marker.exec(url.hash)
      if (!match || match[1] !== intentId) return null
      url.hash = url.hash.slice(0, match.index)
      return url.href === targetUrl ? { intentId, targetUrl } : null
    } catch { return null }
  }

  async function native(request) {
    try { return await api.runtime.sendNativeMessage(host, request) }
    catch { return { status: 'unavailable' } }
  }

  api.runtime.onMessage.addListener((message, sender, respond) => {
    if (!message || sender.id !== api.runtime.id || !sender.tab || sender.frameId !== 0) return false
    if (message.type === 'veyra_hello') {
      if (typeof sender.url !== 'string' || !sender.url.startsWith('https://')) return false
      native({ type: 'hello' }).then(() => respond({ status: 'ready' }))
      return true
    }
    if (message.type === 'veyra_take') {
      const target = verifiedTarget(sender, message.intentId, message.targetUrl)
      if (!target) return false
      native({ type: 'take', intent_id: target.intentId, target_url: target.targetUrl }).then(respond)
      return true
    }
    if (message.type === 'veyra_ack') {
      // Chromium can retain the document's initial URL in MessageSender after
      // replaceState. Accept that exact marked URL or the restored target.
      const originalSender = verifiedTarget(sender, message.intentId, message.targetUrl)
      const restoredSender = typeof sender.url === 'string' && sender.url === message.targetUrl && sender.url.startsWith('https://')
      if ((!originalSender && !restoredSender) || typeof message.filled !== 'boolean') return false
      native({ type: 'ack', intent_id: message.intentId, target_url: message.targetUrl, filled: message.filled }).then(respond)
      return true
    }
    return false
  })
  void native({ type: 'hello' })
})()
