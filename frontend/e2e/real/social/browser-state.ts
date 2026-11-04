import { openRealIdentity } from './real-entry'
import { createPublicJourney } from './journey'
import type { SocialAvailabilityPort, SocialContext } from '../../../src/app/social-composition/ports'
import type { SourcePins } from './observations'

let identity: Awaited<ReturnType<typeof openRealIdentity>> | null = null
let journey: ReturnType<typeof createPublicJourney> | null = null

/** Module lifetime is isolated by a real Playwright browser context, not a mocked session. */
export async function open(input: Parameters<typeof openRealIdentity>[0], source: SourcePins) {
  if (identity) throw new Error('IN03 actor already owns an identity')
  identity = await openRealIdentity(input)
  return identity.bootstrapPacket(source)
}
export function bind(availability: (context: SocialContext) => SocialAvailabilityPort | null) {
  if (!identity || journey) throw new Error('IN03 actor binding unavailable')
  journey = createPublicJourney(identity, identity.bindSocial(availability))
}
export function current() {
  if (!journey) throw new Error('NOT_RUN: no bound production social actor')
  return journey
}
export function http() { return identity?.http() ?? [] }
export function close() { identity?.dispose(); journey = null; identity = null }
