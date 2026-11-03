import { MapError, requireMap } from '../../../world/map/json'
import type { LoadedMap } from '../../../world/map/resources'
import type { WorldRuntime } from '../../../world/runtime'
import { isThemeId, type ThemeId, type ThemePreferences, type ThemeScope } from './preferences'

export interface ThemeState {
  readonly selected: ThemeId
  readonly pending: ThemeId | null
  readonly phase: 'ready' | 'loading' | 'error' | 'disposed'
  readonly message: string
  readonly saved: boolean
}
export interface ThemeOptions {
  scope: ThemeScope
  preferences: ThemePreferences
  runtime: Pick<WorldRuntime, 'readSkin' | 'replaceSkin'>
  /** Use retained loadMap + release pins and approved resource allowlist. Never fabricate absent art. */
  load(theme: ThemeId, signal: AbortSignal): Promise<LoadedMap>
  initial: ThemeId
}
/** One controller per authenticated shell identity/event; dispose before transferring ownership. */
export function createThemeController(options: ThemeOptions) {
  const scope = Object.freeze({ ...options.scope })
  requireMap(Boolean(scope.userId && scope.eventId) && isThemeId(options.initial))
  let state: ThemeState = Object.freeze({ selected: options.initial, pending: null, phase: 'ready', message: '', saved: false })
  let flight: AbortController | null = null, disposed = false, revision = 0
  const listeners = new Set<() => void>()
  const publish = (value: ThemeState) => {
    state = Object.freeze(value)
    for (const listener of Array.from(listeners)) { try { listener() } catch { /* Observer does not own the transaction. */ } }
  }
  async function select(theme: ThemeId) {
    if (disposed || !isThemeId(theme)) return
    flight?.abort()
    const controller = new AbortController(), ticket = ++revision
    flight = controller
    const current = () => !disposed && revision === ticket && !controller.signal.aborted
    const active = options.runtime.readSkin()
    publish({ ...state, selected: isThemeId(active) ? active : state.selected, phase: 'loading', pending: theme, message: `Loading ${theme}…` })
    try {
      if (!current()) return
      if (options.runtime.readSkin() !== theme) {
        await options.runtime.replaceSkin(async signal => {
          const candidate = await options.load(theme, signal)
          if (candidate.theme.theme_id !== theme) { candidate.dispose(); throw new MapError('invalid') }
          return candidate
        }, controller.signal)
      }
      if (!current()) return
      // Local storage is synchronous: no late network write can commit an obsolete choice.
      let saved = true
      try { options.preferences.write(scope, theme) } catch { saved = false }
      if (!current()) return
      publish({ selected: theme, pending: null, phase: saved ? 'ready' : 'error', saved,
        message: saved ? `${theme} selected. Saved on this device for this account and event.` : `${theme} selected. Could not save on this device. Choose it again to retry.` })
    } catch (error) {
      if (!current()) return
      const message = error instanceof MapError && error.reason === 'unavailable'
        ? `${theme} artwork is unavailable. ${state.selected} remains selected.`
        : `Could not load ${theme}. ${state.selected} remains selected. Try again.`
      publish({ ...state, pending: null, phase: 'error', message })
    } finally { if (flight === controller) flight = null }
  }
  return {
    read: () => state,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener) } },
    select,
    async restore() {
      if (disposed || revision !== 0) return
      let saved: unknown
      try { saved = options.preferences.read(scope) }
      catch { publish({ ...state, phase: 'error', message: 'Could not read the saved era. Choose an era to try again.' }); return }
      if (isThemeId(saved)) await select(saved)
    },
    dispose() {
      if (disposed) return
      disposed = true; ++revision; flight?.abort(); flight = null
      publish({ ...state, pending: null, phase: 'disposed', message: '' }); listeners.clear()
    },
  }
}
export type ThemeController = ReturnType<typeof createThemeController>
