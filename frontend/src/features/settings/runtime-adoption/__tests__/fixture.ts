import { createSessionController } from '../../../auth/model/session'
import { createMeController } from '../../../me'
import { participant, eventId, userId } from '../../../me/__tests__/fixtures'
import { createThemeController } from '../../../theme/model/controller'
import { createRuntimePreferencesAdapter, type MotionPreference } from '..'
import { MapError } from '../../../../world/map/json'

/** Deterministic canonical-controller fixture. Effects are instrumented, not actual Phaser/audio. */
export function fixture(motion?: MotionPreference) {
  let saved = participant(), effectCalls = 0, soundCalls = 0, activeEffects = 0, activeSound = 0, writes = 0
  const auth = createSessionController({ eventId, fetch: async input => {
    const request = input as Request
    if (request.method === 'PATCH') {
      const body = await request.json()
      saved = { ...saved, revision: saved.revision + 1, preferences: body.preferences }; writes++
    }
    return new Response(JSON.stringify(saved), { headers: { 'Content-Type': 'application/json' } })
  } })
  auth.session.replace(0, { userId, eventId, accessToken: 'FE34_RUNTIME_SYNTHETIC_ONLY' })
  const controller = createMeController(auth), scope = auth.privacy.capture(), lifetime = new AbortController()
  const theme = createThemeController({ initial: 'REVENGE', scope: { userId, eventId }, preferences: { read: () => null, write() {} },
    runtime: { readSkin: () => 'REVENGE', replaceSkin: async () => { throw new MapError('unavailable') } }, load: async () => { throw new MapError('unavailable') } })
  const lease = (signal: AbortSignal, kind: 'effects' | 'sound') => {
    if (kind === 'effects') { effectCalls++; activeEffects++ } else { soundCalls++; activeSound++ }
    let disposed = false
    const dispose = () => { if (disposed) return; disposed = true; signal.removeEventListener('abort', dispose); if (kind === 'effects') activeEffects--; else activeSound-- }
    signal.addEventListener('abort', dispose, { once: true })
    return { dispose }
  }
  const adapter = createRuntimePreferencesAdapter({ privacy: auth.privacy, scope, signal: lifetime.signal, theme,
    ...(motion ? { motion } : {}), effects: { motion: true, lowPower: true, acquire: (_value, signal) => lease(signal, 'effects') },
    sound: { acquire: signal => lease(signal, 'sound') } })
  return { auth, controller, theme, adapter, lifetime,
    read: () => ({ effectCalls, soundCalls, activeEffects, activeSound, writes, adoption: adapter.read() }),
    revoke() { auth.privacy.changePolicy(scope, 'consent-revoked') },
    dispose() { adapter.dispose(); controller.dispose(); theme.dispose(); auth.dispose() } }
}
