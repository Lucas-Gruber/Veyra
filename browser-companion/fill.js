/* SPDX-License-Identifier: GPL-3.0-only */
(() => {
  'use strict'
  const marker = /(?:^#|&)__veyra_fill=([0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12})$/

  function parseTarget(input) {
    try {
      const url = new URL(input)
      if (url.protocol !== 'https:' || url.username || url.password) return null
      const match = marker.exec(url.hash)
      if (!match) return null
      const originalFragment = url.hash.slice(0, match.index)
      url.hash = originalFragment
      return { intentId: match[1], targetUrl: url.href }
    } catch {
      return null
    }
  }

  function loginFields(targetUrl) {
    if (window.top !== window || location.protocol !== 'https:' || location.href !== targetUrl) return null
    const visible = element => {
      if (!(element instanceof HTMLInputElement) || element.disabled || element.readOnly || !element.isConnected) return false
      if (!element.getClientRects().length) return false
      for (let ancestor = element; ancestor; ancestor = ancestor.parentElement) {
        const style = getComputedStyle(ancestor)
        if (style.visibility !== 'visible' || style.display === 'none' || Number(style.opacity) === 0) return false
      }
      return true
    }
    const passwords = [...document.querySelectorAll('input[type="password"]')]
      .filter(input => visible(input) && input.autocomplete !== 'new-password')
    // Ambiguous login forms and registration/password-change forms fail closed.
    if (passwords.length !== 1) return null
    const password = passwords[0]
    const form = password.form
    if (!form || form.querySelector('input[autocomplete="new-password"]')) return null
    try {
      const action = new URL(form.action || location.href, location.href)
      if (action.protocol !== 'https:' || action.origin !== location.origin) return null
    } catch { return null }
    const inputs = [...form.querySelectorAll('input')]
    const candidates = inputs.filter(input => visible(input) && ['text', 'email', 'tel'].includes(input.type))
    const identified = candidates.filter(input => input.autocomplete === 'username' || input.type === 'email')
    const preceding = candidates.filter(input => inputs.indexOf(input) < inputs.indexOf(password))
    const usernames = identified.length ? identified : preceding
    if (usernames.length !== 1) return null
    const username = usernames[0]
    return { username, password, form, visible }
  }

  function fillLogin(credentials, targetUrl) {
    if (!credentials || typeof credentials.username !== 'string' || typeof credentials.password !== 'string' || !credentials.password) return false
    const fields = loginFields(targetUrl)
    if (!fields) return false
    const { username, password, form, visible } = fields
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set
    if (!setter) return false
    // Use native value setters for controlled forms. Never submit or click.
    setter.call(username, credentials.username)
    username.dispatchEvent(new Event('input', { bubbles: true }))
    username.dispatchEvent(new Event('change', { bubbles: true }))
    // Event handlers may navigate or change the destination between fields.
    if (location.href !== targetUrl || !visible(password) || password.form !== form || new URL(form.action || location.href, location.href).origin !== location.origin) return false
    setter.call(password, credentials.password)
    password.dispatchEvent(new Event('input', { bubbles: true }))
    password.dispatchEvent(new Event('change', { bubbles: true }))
    return location.href === targetUrl && username.value === credentials.username && password.value === credentials.password
  }

  globalThis.VeyraFill = Object.freeze({ parseTarget, fillLogin, canFill: target => Boolean(loginFields(target)) })
})()
