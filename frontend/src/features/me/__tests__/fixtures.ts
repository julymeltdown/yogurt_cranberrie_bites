import { QueryClient } from '@tanstack/react-query'
import { createMemorySession } from '../../../shared/api/http'
import { createQueryPrivacy } from '../../../shared/query/privacy'
import { createSessionController } from '../../auth/model/session'
import { createMeController } from '../model/controller'
import config from '../../../../../contracts/fixtures/assets/avatar-config.json'
export const userId = '11111111-1111-4111-8111-111111111111', eventId = '22222222-2222-4222-8222-222222222222'
export function participant() { return { event_id: eventId, user_id: userId, revision: 1,
  profile: { user_id: userId, display_name: 'Synthetic listener', bio: null, avatar: { revision: 1, config: structuredClone(config) } },
  preferences: { theme: 'REVENGE' as const, low_power: false, reduced_motion: false, sound_enabled: false }, age_status: 'UNKNOWN' as const } }
export function setup(fetcher: typeof fetch) {
  const auth = createSessionController({ eventId, fetch: fetcher })
  auth.session.replace(0, { userId, eventId, accessToken: 'SYNTHETIC_ONLY' })
  const controller = createMeController(auth)
  return { auth, controller, dispose() { controller.dispose(); auth.dispose() } }
}
export function privacyOnly() {
  const session = createMemorySession(), client = new QueryClient(), privacy = createQueryPrivacy({ session, client })
  session.replace(0, { userId, eventId, accessToken: 'SYNTHETIC_ONLY' })
  return { session, privacy, client }
}
