import { isTauri, invoke } from '@tauri-apps/api/core'

export interface AppInformation {
  version: string
  channel: 'stable' | 'prerelease'
  platform: string
  architecture: string
  license: string
  repositoryUrl: string
}
export interface UpdateResult {
  state: 'up_to_date' | 'available' | 'no_releases'
  currentVersion: string
  latestVersion: string | null
  releaseUrl: string | null
  publishedAt: string | null
  checkedAt: string
}
interface UpdateError { code: string; message: string }
const CHECK_INTERVAL = 24 * 60 * 60 * 1000
const AUTOMATIC_RETRY_INTERVAL = 60 * 60 * 1000
const CACHE_KEY = 'veyra:update-check'
let infoRequest: Promise<void> | undefined
let checkRequest: Promise<UpdateResult | null> | undefined

function safeError(error: unknown, fallback: string): UpdateError {
  if (error && typeof error === 'object' && 'code' in error && 'message' in error && typeof error.code === 'string' && typeof error.message === 'string') {
    return { code: error.code, message: error.message }
  }
  return { code: 'check_unavailable', message: fallback }
}

export function useAppUpdates() {
  const config = useAppConfig()
  const prefs = usePreferences()
  const isDesktop = computed(() => import.meta.client && isTauri())
  const buildVersion = computed(() => typeof config.appVersion === 'string' ? config.appVersion : 'Unknown')
  const info = useState<AppInformation | null>('updates:app-info', () => null)
  const version = computed(() => info.value?.version || buildVersion.value)
  const checking = useState('updates:checking', () => false)
  const result = useState<UpdateResult | null>('updates:result', () => null)
  const error = useState<UpdateError | null>('updates:error', () => null)
  const infoError = useState<string | null>('updates:info-error', () => null)
  const initialized = useState('updates:initialized', () => false)
  const lastAttemptAt = useState<string | null>('updates:last-attempt', () => null)
  const automaticAttemptAt = useState('updates:automatic-attempt', () => 0)

  function restoreSuccessfulCheck() {
    try {
      const cached = JSON.parse(localStorage.getItem(CACHE_KEY) || 'null') as UpdateResult | null
      if (!cached || cached.currentVersion !== version.value || !['up_to_date', 'available', 'no_releases'].includes(cached.state)) return
      if (typeof cached.checkedAt !== 'string' || cached.checkedAt.length > 64) return
      const timestamp = Date.parse(cached.checkedAt)
      if (!Number.isFinite(timestamp) || timestamp > Date.now() + 5 * 60 * 1000) return
      if (cached.latestVersion !== null && (typeof cached.latestVersion !== 'string' || cached.latestVersion.length > 128)) return
      if (cached.state === 'available' && !cached.latestVersion) return
      if (cached.state === 'no_releases' && cached.latestVersion !== null) return
      if (cached.releaseUrl !== null && (typeof cached.releaseUrl !== 'string' || cached.releaseUrl.length > 512)) return
      if (cached.publishedAt !== null && (typeof cached.publishedAt !== 'string' || cached.publishedAt.length > 64 || !Number.isFinite(Date.parse(cached.publishedAt)))) return
      // Persisted data only informs display/backoff. Opening a release requires
      // another native check, which validates the version and fixed repository.
      result.value = { state: cached.state, currentVersion: cached.currentVersion, latestVersion: cached.latestVersion, releaseUrl: cached.releaseUrl, publishedAt: cached.publishedAt, checkedAt: cached.checkedAt }
    } catch { /* Invalid or unavailable preference storage does not prevent checks. */ }
  }

  async function initialize(): Promise<void> {
    if (!isDesktop.value || initialized.value) return
    if (infoRequest) return infoRequest
    infoRequest = (async () => {
      try {
        info.value = await invoke<AppInformation>('app_info')
        infoError.value = null
        initialized.value = true
        restoreSuccessfulCheck()
      } catch {
        infoError.value = 'Native program information is unavailable. Please restart Veyra.'
      }
    })().finally(() => { infoRequest = undefined })
    return infoRequest
  }

  async function checkForUpdates(): Promise<UpdateResult | null> {
    if (!isDesktop.value) return null
    if (checkRequest) return checkRequest
    checkRequest = (async () => {
      checking.value = true
      error.value = null
      lastAttemptAt.value = new Date().toISOString()
      try {
        await initialize()
        if (!info.value) throw { code: 'check_unavailable', message: 'Could not read the installed version. Please restart Veyra.' }
        const checked = await invoke<UpdateResult>('check_for_updates')
        if (checked.currentVersion !== info.value.version) throw { code: 'check_unavailable', message: 'The running version changed. Please restart Veyra before checking again.' }
        result.value = checked
        try { localStorage.setItem(CACHE_KEY, JSON.stringify(checked)) }
        catch { /* A successful check remains useful without persistence. */ }
        return checked
      } catch (failure) {
        error.value = safeError(failure, 'Could not check for updates. Check your connection and try again.')
        return null
      } finally { checking.value = false }
    })().finally(() => { checkRequest = undefined })
    return checkRequest
  }

  async function checkAutomaticallyIfDue() {
    if (!isDesktop.value || !prefs.automaticUpdateChecks) return
    await initialize()
    if (!info.value) return
    const lastSuccess = result.value ? Date.parse(result.value.checkedAt) : 0
    if (Date.now() - lastSuccess < CHECK_INTERVAL || Date.now() - automaticAttemptAt.value < AUTOMATIC_RETRY_INTERVAL) return
    automaticAttemptAt.value = Date.now()
    await checkForUpdates()
  }

  function startAutomaticChecks(): () => void {
    if (!isDesktop.value || new URLSearchParams(window.location.search).get('window') === 'quick') return () => {}
    void initialize().then(checkAutomaticallyIfDue)
    const interval = setInterval(() => { void checkAutomaticallyIfDue() }, 60_000)
    return () => clearInterval(interval)
  }

  function setAutomaticChecks(enabled: boolean) {
    prefs.automaticUpdateChecks = enabled
    prefs.persist()
    if (enabled) void checkAutomaticallyIfDue()
  }

  async function openRelease() {
    // A cached browser preference is not authority to open a URL. The backend
    // rechecks its own candidate and opens only this project's validated page.
    const checked = await checkForUpdates()
    if (!checked?.latestVersion) return
    try { await invoke('open_release_page', { version: checked.latestVersion }) }
    catch (failure) { error.value = safeError(failure, 'Could not open the release page. Try checking again.') }
  }

  return { info, version, buildVersion, isDesktop, checking, result, error, infoError, lastAttemptAt, initialize, checkForUpdates, openRelease, startAutomaticChecks, setAutomaticChecks }
}
