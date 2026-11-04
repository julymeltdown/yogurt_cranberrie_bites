// @vitest-environment jsdom
import { expect, it } from 'vitest'
import { QueryClient } from '@tanstack/react-query'
import { createHttpClient, createMemorySession } from '../../../../shared/api/http'
import { createQueryPrivacy } from '../../../../shared/query/privacy'
import { createMuralToolsHttp } from '../tools-http'
import type { WallActionCommand } from '../../editor/tools'
import { authorityPort, ids } from './fixture'
import qualified from './wall-tools-b-wire/tools-qualified.json'
import ownFill from './wall-tools-b-wire/own-fill-256.json'

function fixture() {
  const session = createMemorySession(), client = new QueryClient(), authority = authorityPort(), requests: Request[] = []
  session.replace(0, { userId: qualified.actor_id, eventId: qualified.event_id, accessToken: 'fixture-only-memory' })
  authority.set({ binding: { eventId: qualified.event_id, roomId: ids.room, roomEpoch: ids.epoch, streamId: ids.stream }, writable: true })
  const privacy = createQueryPrivacy({ session, client })
  let monotonic = 0
  let respond = async (_request: Request): Promise<Response> => Response.json(ownFill)
  const http = createHttpClient({ session, fetch: async input => { const request = input as Request; requests.push(request.clone()); return respond(request) } })
  const adapter = createMuralToolsHttp({ http, privacy, authority, now: () => Date.parse(qualified.server_time), monotonicNow: () => monotonic })
  const command: WallActionCommand = { scope: privacy.capture(), key: ownFill.idempotency_key, operation: 'fill',
    body: { expected_revision: 1, x: 0, y: 0, color_index: 2 } }
  const receipt = { receipt_id: ownFill.receipt_id, event_id: qualified.event_id, actor_id: qualified.actor_id,
    operation_id: 'fillConcertMuralRegion', idempotency_key: command.key, state: 'SUCCEEDED', resource_id: qualified.event_id,
    resource_revision: 2, failure_reason: null, created_at: qualified.server_time, expires_at: '2026-11-06T00:00:00Z', result_status: 200 }
  return { session, authority, privacy, requests, adapter, command, receipt, signal: () => new AbortController().signal,
    elapse(ms: number) { monotonic += ms }, respond(fn: typeof respond) { respond = fn }, dispose() { privacy.dispose(); client.clear() } }
}
it.each([4999, 5000])('uses monotonic request-start freshness for a %ims capability response', async elapsed => {
  const f = fixture()
  try {
    f.respond(async () => { f.elapse(elapsed); return Response.json(qualified) })
    const result = f.adapter.load(f.command.scope, f.signal())
    if (elapsed < 5000) await expect(result).resolves.toMatchObject({ version: 1 })
    else await expect(result).rejects.toThrow()
  } finally { f.dispose() }
})
it('normalizes the frozen private tools fixture through the actual shared transport without caching', async () => {
  const f = fixture()
  try {
    f.respond(async () => Response.json(qualified))
    expect(await f.adapter.load(f.command.scope, f.signal())).toMatchObject({ version: 1, actorId: qualified.actor_id,
      capabilities: { backgroundIndex: 0, pencil: true }, budget: { remaining: 256, nextCellAvailableAt: null }, undoTarget: { changedCells: 256 } })
    expect(f.requests).toHaveLength(1); expect(f.requests[0]!.cache).toBe('no-store')
    expect(new URL(f.requests[0]!.url).search).toBe(''); expect(f.requests[0]!.headers.get('Authorization')).toBe('Bearer fixture-only-memory')
  } finally { f.dispose() }
})
it.each(['foreign actor', 'foreign latest', 'wrong undo', 'extra field', 'wrong limits'])('rejects %s in a private context', async variant => {
  const f = fixture()
  try {
    const wire = structuredClone(qualified)
    if (variant === 'foreign actor') wire.actor_id = ids.other
    if (variant === 'foreign latest') wire.latest_action.actor_id = ids.other
    if (variant === 'wrong undo') wire.undo.action_id = ids.other
    if (variant === 'wrong limits') wire.fill_limit = 257
    f.respond(async () => Response.json(variant === 'extra field' ? { ...wire, token: 'not-permitted' } : wire))
    await expect(f.adapter.load(f.command.scope, f.signal())).rejects.toThrow()
  } finally { f.dispose() }
})
it('sends one exact immutable fill body/key and verifies the full own-action binding', async () => {
  const f = fixture()
  try {
    expect(await f.adapter.submit(f.command, f.signal())).toMatchObject({ kind: 'acknowledged', receipt: { revision: 2, changedCells: 256, resetRequired: true } })
    expect(f.requests).toHaveLength(1); const request = f.requests[0]!
    expect(request.method).toBe('POST'); expect(new URL(request.url).pathname.endsWith('/mural/fill')).toBe(true)
    expect(request.headers.get('Idempotency-Key')).toBe(f.command.key); expect(await request.json()).toEqual(f.command.body)
  } finally { f.dispose() }
})
it('accepts an eligible undo expiry clipped by retained receipt lifetime', async () => {
  const f = fixture()
  try {
    f.respond(async () => Response.json({ ...ownFill, undo_expires_at: '2026-11-05T00:04:00.123456Z' }))
    expect(await f.adapter.submit(f.command, f.signal())).toMatchObject({ kind: 'acknowledged' })
  } finally { f.dispose() }
})
it.each(['foreign actor', 'altered body', 'wrong key', 'wrong receipt kind', 'missing reset'])('keeps uncertain %s action responses unresolved', async variant => {
  const f = fixture()
  try {
    const wire = structuredClone(ownFill)
    if (variant === 'foreign actor') wire.actor_id = ids.other
    if (variant === 'altered body') wire.command.body.x = 1
    if (variant === 'wrong key') wire.idempotency_key = ids.other
    if (variant === 'wrong receipt kind') wire.kind = 'UNDO'
    if (variant === 'missing reset') wire.reset_required = false
    f.respond(async () => Response.json(wire))
    expect(await f.adapter.submit(f.command, f.signal())).toEqual({ kind: 'unknown' })
  } finally { f.dispose() }
})
it('checks the original receipt then own action using GET only and the same exact identity', async () => {
  const f = fixture()
  try {
    f.respond(async request => Response.json(new URL(request.url).pathname.endsWith('/commands') ? f.receipt : ownFill))
    expect(await f.adapter.check(f.command, f.signal())).toMatchObject({ kind: 'acknowledged' })
    expect(f.requests.map(r => r.method)).toEqual(['GET', 'GET'])
    expect(new URL(f.requests[0]!.url).searchParams.get('operation_id')).toBe('fillConcertMuralRegion')
    const own = new URL(f.requests[1]!.url); expect(own.pathname.endsWith(`/mural/actions/${f.command.key}`)).toBe(true); expect(own.search).toBe('')
  } finally { f.dispose() }
})
it('rejects an own result with a different receipt despite matching key and body', async () => {
  const f = fixture()
  try {
    f.respond(async request => Response.json(new URL(request.url).pathname.endsWith('/commands') ? f.receipt : { ...ownFill, receipt_id: ids.other }))
    expect(await f.adapter.check(f.command, f.signal())).toEqual({ kind: 'unknown' })
  } finally { f.dispose() }
})
it('retains pending receipts and binds typed terminal rejections through registered server errors', async () => {
  const f = fixture()
  try {
    f.respond(async () => Response.json({ ...f.receipt, state: 'PENDING', resource_id: null, resource_revision: null, result_status: null }, { status: 202 }))
    expect(await f.adapter.submit(f.command, f.signal())).toEqual({ kind: 'pending' })
    f.respond(async () => Response.json({ code: 'MCR_429', message: 'Unavailable.', details: { reason: 'MURAL_BUDGET_EXCEEDED' } }, { status: 429 }))
    expect(await f.adapter.submit(f.command, f.signal())).toEqual({ kind: 'rejected', reason: 'MURAL_BUDGET_EXCEEDED' })
  } finally { f.dispose() }
})
it('keeps an ambiguous unresolved POST uncertain until the exact original receipt proves FAILED', async () => {
  const f = fixture()
  try {
    f.respond(async () => Response.json({ code: 'MCR_409', message: 'Unavailable.', details: { reason: 'MURAL_ACTION_UNRESOLVED' } }, { status: 409 }))
    expect(await f.adapter.submit(f.command, f.signal())).toEqual({ kind: 'unknown' })
    f.respond(async () => Response.json({ ...f.receipt, state: 'PENDING', resource_id: null, resource_revision: null, result_status: null }))
    expect(await f.adapter.check(f.command, f.signal())).toEqual({ kind: 'pending' })
    f.respond(async () => Response.json({ ...f.receipt, state: 'FAILED', resource_id: null, resource_revision: null,
      failure_reason: 'MURAL_ACTION_UNRESOLVED', result_status: null }))
    expect(await f.adapter.check(f.command, f.signal())).toEqual({ kind: 'rejected', reason: 'MURAL_ACTION_UNRESOLVED' })
    expect(f.requests.map(r => r.method)).toEqual(['POST', 'GET', 'GET'])
  } finally { f.dispose() }
})
it('recovers exact unresolved legacy pencil metadata without posting or minting a key', async () => {
  const f = fixture()
  try {
    const unresolved = { receipt_id: ownFill.receipt_id, idempotency_key: ownFill.idempotency_key,
      command: { operation_id: 'placeConcertPixel', body: ownFill.command.body } }
    f.respond(async () => Response.json({ ...qualified, unresolved }))
    expect(await f.adapter.load(f.command.scope, f.signal())).toMatchObject({ unresolved: { operation: 'placeConcertPixel', key: ownFill.idempotency_key, body: ownFill.command.body } })
    expect(f.requests.map(r => r.method)).toEqual(['GET'])
  } finally { f.dispose() }
})
it('fences read-only submission and late privacy-invalidated responses', async () => {
  const f = fixture()
  try {
    f.authority.set({ ...f.authority.read()!, writable: false })
    expect(await f.adapter.submit(f.command, f.signal())).toEqual({ kind: 'unknown' }); expect(f.requests).toHaveLength(0)
    f.authority.set({ ...f.authority.read()!, writable: true })
    f.respond(async () => { f.session.replace(f.session.read().generation, null); return Response.json(ownFill) })
    expect(await f.adapter.submit(f.command, f.signal())).toEqual({ kind: 'unknown' })
  } finally { f.dispose() }
})
