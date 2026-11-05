import { QueryClient } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { createMemorySession } from '../../src/shared/api/http'
import { createQueryPrivacy, type PrivacyScope } from '../../src/shared/query/privacy'
import type { MeetingOpenIntent } from '../../src/features/paradise-city-map/ports'
import { landmarkId } from '../../src/features/paradise-city-map/spatial/contract'
import type { LeafCommand, MeetingContextView } from '../../src/features/venue-invitations/model'
import { createMeetingLeafOwner } from '../../src/app/venue-community-entry/owner'
import type { LeafCommandResult, MeetingLeafAdapter, MeetingLeafRelease, LeafRequest } from '../../src/app/venue-community-entry/ports'

export const fixtureIds = {
  actor: '00000000-0000-4000-8000-000000000001', friend: '00000000-0000-4000-8000-000000000002',
  other: '00000000-0000-4000-8000-000000000003', event: '00000000-0000-4000-8000-000000000004',
  invitation: '00000000-0000-4000-8000-000000000005',
} as const
export function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (error: Error) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
/** Controlled presentation fixture only; no actual landmark authority or production wire. */
export function meetingFixture(portraits: readonly [ReactNode, ReactNode] = [null, null], withMapFocus = false) {
  const session = createMemorySession(), client = new QueryClient()
  session.replace(0, { userId: fixtureIds.actor, eventId: fixtureIds.event, accessToken: 'fixture-only' })
  const privacy = createQueryPrivacy({ client, session }), auth = { session, privacy }
  const scope = privacy.capture(), lifetime = new AbortController()
  const focuses: unknown[] = []
  const clocks = new Set<() => void>(), invalidations = new Set<() => void>()
  let time = 1_800_000_000_000, clockScope: PrivacyScope = scope, clockAvailable = true, keys = 0
  const place = { landmarkId: 'synthetic-real-main-gate', catalogRevision: '7', landmarkRevision: '11' }
  const intent: MeetingOpenIntent = {
    action: 'MESSAGE', scope,
    venue: { venueId: 'fixture-venue', siteplanId: 'fixture-siteplan', revision: 1, variantId: 'fixture-variant' },
    landmark: place, targetUserId: fixtureIds.friend, origin: 'map-friend',
  }
  const inviteIntent: MeetingOpenIntent = { ...intent, action: 'INVITE', ...place }
  let context: MeetingContextView = {
    contextId: 'fixture-opaque-context', contextVersion: 'fixture-context-v1', selection: intent,
    landmarkLabel: 'Main Gate', landmarkDescription: 'Meeting place at Paradise City.',
    self: { userId: fixtureIds.actor, displayName: 'You', portrait: portraits[0] },
    friend: { userId: fixtureIds.friend, displayName: 'inkparade', portrait: portraits[1] },
    validUntilMs: time + 900_000, canRead: true, canSend: true, invitationActions: ['create'], invitation: null,
    messages: [], conversation: null,
    messagePolicy: { hint: 'Fixture limit: 500 Unicode code points.', accepts: body => body.trim().length > 0 && [...body].length <= 500 },
  }
  const reads: { kind: 'selection' | 'invitation'; target: MeetingOpenIntent | string; request: LeafRequest }[] = []
  const writes: { command: LeafCommand; request: LeafRequest }[] = [], statuses: typeof writes = []
  let writeResult: LeafCommandResult = { state: 'applied' }, statusResult: LeafCommandResult = { state: 'applied' }
  let readNext: Promise<MeetingContextView | null> | null = null, writeNext: Promise<LeafCommandResult> | null = null
  let statusNext: Promise<LeafCommandResult> | null = null
  const applied = new Set<string>()
  function apply(command: LeafCommand, result: LeafCommandResult) {
    if (result.state !== 'applied' || applied.has(command.key)) return result
    applied.add(command.key)
    if (command.kind === 'send') context = { ...context, conversation: { id: 'fixture-conversation', earlierCursor: null },
      messages: [...context.messages, { id: command.key, authorId: fixtureIds.actor, body: command.body!, timeLabel: 'Just now' }] }
    else if (command.kind === 'create') context = { ...context, invitation: { id: fixtureIds.invitation, version: '1', status: 'pending', expiresAtMs: time + 600_000 }, invitationActions: ['cancel'] }
    else if (context.invitation) context = { ...context, invitation: { ...context.invitation, version: '2',
      status: command.kind === 'accept' ? 'accepted' : command.kind === 'decline' ? 'declined' : 'cancelled' }, invitationActions: command.kind === 'accept' ? ['cancel'] : [] }
    return result
  }
  const adapter: MeetingLeafAdapter = {
    async readSelection(target, request) { reads.push({ kind: 'selection', target, request }); const next = readNext; readNext = null; return next ? await next : context },
    async readInvitation(target, request) { reads.push({ kind: 'invitation', target, request }); const next = readNext; readNext = null; return next ? await next : context },
    async execute(command, request) { writes.push({ command, request }); const next = writeNext; writeNext = null; return apply(command, next ? await next : writeResult) },
    async status(command, request) { statuses.push({ command, request }); const next = statusNext; statusNext = null; return apply(command, next ? await next : statusResult) },
  }
  const release: MeetingLeafRelease = { scope, signal: lifetime.signal, adapter, classification: 'CONTROLLED_FIXTURE',
    clock: { read: () => clockAvailable ? { scope: clockScope, nowMs: time } : null,
      subscribe(fn) { clocks.add(fn); return () => { clocks.delete(fn) } } },
    invalidations: { subscribe(fn) { invalidations.add(fn); return () => { invalidations.delete(fn) } } },
    ...(withMapFocus ? { mapVenue: { identity: intent.venue, title: 'Controlled illustrated focus surface',
      image: { src: '', width: 100, height: 100, description: 'Controlled fixture coordinates, not a real venue position' },
      classification: 'ILLUSTRATED_UNCALIBRATED' as const, landmarks: [{ id: landmarkId('synthetic-art-gate'), label: 'Main Gate',
        point: { x: 50, y: 50 }, meetingContext: place }] }, mapFocus: { focus(value: unknown) { focuses.push(value); return true } } } : {}),
  }
  const owner = createMeetingLeafOwner({ auth, release, newCommandKey: () => `00000000-0000-4000-8000-${String(++keys).padStart(12, '0')}` })
  const clockChanged = () => { for (const fn of Array.from(clocks)) fn() }
  return { auth, scope, release, lifetime, intent, inviteIntent, owner, reads, writes, statuses, focuses,
    context: () => context, patch(patch: Partial<MeetingContextView>) { context = { ...context, ...patch } },
    setWriteResult(result: LeafCommandResult) { writeResult = result }, setStatusResult(result: LeafCommandResult) { statusResult = result },
    nextRead(promise: Promise<MeetingContextView | null>) { readNext = promise },
    nextWrite(promise: Promise<LeafCommandResult>) { writeNext = promise },
    nextStatus(promise: Promise<LeafCommandResult>) { statusNext = promise },
    advance(ms: number) { time += ms; clockChanged() }, time: () => time,
    loseClock() { clockAvailable = false; clockChanged() }, replaceClockScope(next: PrivacyScope) { clockScope = next; clockChanged() },
    invalidate() { for (const fn of Array.from(invalidations)) fn() },
    subscriptions: () => ({ clock: clocks.size, invalidation: invalidations.size }),
    dispose() { owner.dispose(); privacy.dispose(); client.clear() },
  }
}
