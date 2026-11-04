import { openRealIdentity } from './real-entry'
import { createPublicJourney } from './journey'
import type { SocialContext } from '../../../src/app/social-composition/ports'
import type { IN03AvailabilityPort } from '../../../src/app/in03-entry/availability'
import type { SourcePins } from './observations'

let identity: Awaited<ReturnType<typeof openRealIdentity>> | null = null
let journey: ReturnType<typeof createPublicJourney> | null = null

/** Module lifetime is isolated by a real Playwright browser context, not a mocked session. */
export async function open(input: Parameters<typeof openRealIdentity>[0], source: SourcePins) {
  if (identity) throw new Error('IN03 actor already owns an identity')
  identity = await openRealIdentity(input)
  return identity.bootstrapPacket(source)
}
export async function bind(availability: (context: SocialContext) => IN03AvailabilityPort | null) {
  if (!identity || journey) throw new Error('IN03 actor binding unavailable')
  const actor = identity, social = await actor.bindSocial(availability)
  if (identity !== actor) { social.dispose(); throw new Error('IN03 actor closed during preparation') }
  journey = createPublicJourney(actor, social)
}
export async function rebind(availability: (context: SocialContext) => IN03AvailabilityPort | null) {
  if (!identity) throw new Error('IN03 actor unavailable')
  const actor = identity
  journey?.dispose(); journey = null
  const social = await actor.reopenSocial(availability)
  if (identity !== actor) { social.dispose(); throw new Error('IN03 actor closed during rebind') }
  journey = createPublicJourney(actor, social)
}
export function current() {
  if (!journey) throw new Error('NOT_RUN: no bound production social actor')
  return journey
}
export function http() { return identity?.http() ?? [] }
export function close() { journey?.dispose(); identity?.dispose(); journey = null; identity = null }
