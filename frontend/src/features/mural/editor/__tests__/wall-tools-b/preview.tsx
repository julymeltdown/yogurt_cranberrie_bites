import { createRoot } from 'react-dom/client'
import { createMuralSync } from '../../../sync/controller'
import { createMuralToolsHttp } from '../../../sync/tools-http'
import { createHttpClient } from '../../../../../shared/api/http'
import { MuralSyncPanel } from '../../../sync/MuralSyncPanel'
import { createMuralTextureSink } from '../../../../../world/mural/sink'
import { authorityPort, delta, httpClientFixture, memoryScene, serverFixture } from '../../../sync/__tests__/fixture'
import { actionAck, island, toolContext, ids } from './fixture'
import { actionReceiptSchema, previewWallAction, recoveredCommandSchema, toolContextSchema } from '../../tools'
import type { RecoveredCommand, WallActionCommand, WallActionResult, WallToolPort } from '../../tools'
import type { MuralSnapshot } from '../../contract'
import '../reference-target/preview.css'

// Deterministic HTTP fixture, not product data or a substitute backend.
// One canonical in-memory session/privacy owner survives editor reconstruction.
const authority = authorityPort(), client = httpClientFixture(serverFixture(), undefined, authority)
const root = createRoot(document.getElementById('root')!)
let server = island(), remaining = 512, posts = 0, lost = false, serial = 700
const competingKey = '00000000-0000-4000-8000-000000000999'
let competing = false, competingOriginal: WallActionCommand | null = null
let blockedAttempt: { command: WallActionCommand; reads: number } | null = null
let unresolved: RecoveredCommand | null = null
let last: { id: string; revision: number; cells: number; before: number[] } | null = null
const results = new Map<string, WallActionResult>()
const ownResults = new Map<string, ReturnType<typeof wireAction>>()
let latestOwn: ReturnType<typeof wireAction> | null = null
const tools: WallToolPort = {
  async load() {
    return toolContext({ revision: server.revision, budget: { limit: 512, windowMs: 600000, remaining, nextCellAvailableAt: '2026-11-04T00:10:00Z' },
      undoTarget: last ? { actionId: last.id, revision: last.revision, changedCells: last.cells, expiresAt: '2026-11-04T00:10:00Z' } : null,
      unresolved })
  },
  async submit(command) {
    if (command.body.expected_revision !== server.revision) return { kind: 'rejected', reason: 'VERSION_CONFLICT' }
    const before = [...server.pixels], pixels = [...before]
    let count = 0
    if (command.operation === 'undo') {
      if (!last || last.id !== command.body.target_action_id || last.revision !== server.revision) return { kind: 'rejected', reason: 'VERSION_CONFLICT' }
      for (let i = 0; i < pixels.length; i++) if (pixels[i] !== last.before[i]) { pixels[i] = last.before[i]!; count++ }
    } else {
      const draft = previewWallAction(server, command.operation, command.body.x, command.body.y, command.operation === 'erase' ? 0 : command.body.color_index)
      if (!draft.ok) return { kind: 'rejected', reason: draft.reason }
      count = draft.draft.count
      for (const cell of draft.draft.cells) pixels[cell] = command.operation === 'erase' ? 0 : command.body.color_index
    }
    if (count > remaining) return { kind: 'rejected', reason: 'MURAL_BUDGET_EXCEEDED' }
    remaining -= count
    server = { ...server, pixels, revision: server.revision + 1 }
    const actionId = `00000000-0000-4000-8000-${String(++serial).padStart(12, '0')}`
    last = command.operation === 'undo' ? null : { id: actionId, revision: server.revision, cells: count, before }
    const ack = actionAck(command, { actionId, changedCells: count })
    results.set(command.key, ack)
    if (lost) { const { operation, key, body } = command; unresolved = recoveredCommandSchema.parse({ operation, key, body }); return { kind: 'unknown' } }
    return ack
  },
  async check(command: WallActionCommand) {
    if (unresolved && (unresolved.key !== command.key || JSON.stringify(unresolved.body) !== JSON.stringify(command.body))) return { kind: 'unknown' }
    unresolved = null
    return results.get(command.key) ?? { kind: 'unknown' }
  },
}
const operationIds = { erase: 'eraseConcertMuralPixel', fill: 'fillConcertMuralRegion', undo: 'undoConcertMuralAction' } as const
function wireAction(command: WallActionCommand) {
  const result = results.get(command.key)
  if (result?.kind !== 'acknowledged') throw new Error('Missing fixture action')
  const receipt = actionReceiptSchema.parse(result.receipt)
  return { action_id: receipt.actionId, receipt_id: command.key === competingKey ? competingKey : receipt.actionId, event_id: ids.event, actor_id: ids.user,
    command: { operation_id: operationIds[command.operation], body: command.body }, idempotency_key: command.key,
    kind: command.operation.toUpperCase(), revision: receipt.revision, changed_cells: receipt.changedCells,
    committed_at: receipt.committedAt, next_allowed_at: receipt.nextAllowedAt,
    undo_expires_at: '2026-11-04T00:10:00Z', reset_required: true }
}
const http = createHttpClient({ session: client.session, fetch: async input => {
  const request = input as Request, url = new URL(request.url)
  if (request.headers.get('Authorization') !== `Bearer ${ids.user}`) return Response.json({ code: 'MCR_401', message: 'Unavailable.', details: { reason: 'AUTHENTICATION_REQUIRED' } }, { status: 401 })
  if (request.method === 'GET' && url.pathname.endsWith('/mural/tools')) {
    const context = toolContextSchema.parse(await tools.load(client.privacy.capture(), request.signal))
    return Response.json({ version: 1, event_id: ids.event, actor_id: ids.user, revision: server.revision, server_time: context.serverTime,
      background_index: context.capabilities.backgroundIndex, fill_limit: 256, cell_allowance: 512, window_seconds: 600,
      remaining_cells: remaining, next_cell_available_at: context.budget.nextCellAvailableAt, next_allowed_at: null,
      editable: server.state === 'EDITABLE', available: { pencil: true, erase: true, fill: true, undo: true }, undo_window_seconds: 600,
      undo: last ? { action_id: last.id, revision: last.revision, changed_cells: last.cells, expires_at: '2026-11-04T00:10:00Z' } : null,
      latest_action: latestOwn,
      unresolved: unresolved ? { command: { operation_id: unresolved.operation === 'placeConcertPixel' ? 'placeConcertPixel' : operationIds[unresolved.operation], body: unresolved.body },
        idempotency_key: unresolved.key, receipt_id: ownResults.get(unresolved.key)?.receipt_id ?? (unresolved.key === competingKey ? competingKey : undefined) } : null })
  }
  if (request.method === 'GET' && url.pathname.endsWith('/commands')) {
    const key = url.searchParams.get('idempotency_key') ?? '', result = ownResults.get(key)
    if (blockedAttempt?.command.key === key) {
      const state = ++blockedAttempt.reads === 1 ? 'PENDING' : 'FAILED'
      return Response.json({ receipt_id: key, event_id: ids.event, actor_id: ids.user,
        operation_id: operationIds[blockedAttempt.command.operation], idempotency_key: key, state, resource_id: null,
        resource_revision: null, failure_reason: state === 'FAILED' ? 'MURAL_ACTION_UNRESOLVED' : null,
        created_at: '2026-11-04T00:00:00Z', expires_at: '2026-11-05T00:00:00Z', result_status: null })
    }
    if (!result) return Response.json({}, { status: 404 })
    return Response.json({ receipt_id: result.receipt_id, event_id: ids.event, actor_id: ids.user,
      operation_id: result.command.operation_id, idempotency_key: key, state: 'SUCCEEDED', resource_id: ids.event,
      resource_revision: result.revision, failure_reason: null, created_at: result.committed_at, expires_at: '2026-11-05T00:00:00Z', result_status: 200 })
  }
  if (request.method === 'GET' && url.pathname.includes('/mural/actions/')) {
    const key = url.pathname.split('/').at(-1)!, result = ownResults.get(key)
    if (result) { unresolved = null; return Response.json(result) }
    return Response.json({}, { status: 404 })
  }
  if (request.method === 'POST') {
    posts++
    const operation = url.pathname.split('/').at(-1)
    if (operation !== 'erase' && operation !== 'fill' && operation !== 'undo') throw new Error('Unexpected fixture operation')
    const body: unknown = await request.json(), key = request.headers.get('Idempotency-Key')!
    const identity = recoveredCommandSchema.parse({ operation, key, body })
    if (identity.operation === 'placeConcertPixel') throw new Error('Wrong fixture tool operation')
    const command = { ...identity, scope: client.privacy.capture() }
    if (competing) {
      blockedAttempt = { command, reads: 0 }
      competingOriginal = { operation: 'erase', key: competingKey, scope: command.scope,
        body: { expected_revision: server.revision, x: 10, y: 10 } }
      unresolved = { operation: 'erase', key: competingKey, body: competingOriginal.body }
      return Response.json({ code: 'MCR_409', message: 'Unavailable.', details: { reason: 'MURAL_ACTION_UNRESOLVED' } }, { status: 409 })
    }
    const result = await tools.submit(command, request.signal)
    if (result.kind === 'rejected') {
      const status = result.reason === 'MURAL_BUDGET_EXCEEDED' ? 429 : 409
      return Response.json({ code: `MCR_${status}`, message: 'Unavailable.', details: { reason: result.reason } }, { status })
    }
    const own = wireAction(command); ownResults.set(key, own); latestOwn = own
    if (result.kind === 'unknown') throw new Error('Fixture lost response after durable commit')
    return Response.json(own)
  }
  throw new Error('Unexpected fixture request')
} })
const wireTools = createMuralToolsHttp({ http, privacy: client.privacy, authority, now: () => Date.parse('2026-11-04T00:00:00Z') })
function mount() {
  const scene = memoryScene(), sink = createMuralTextureSink(scene.port)
  const sync = createMuralSync({ privacy: client.privacy, authority, sink, tools: wireTools,
    newKey: () => `00000000-0000-4000-8000-${String(++serial).padStart(12, '0')}`,
    reads: { snapshot: async () => structuredClone(server), delta: async request => delta(request.afterRevision, [], request.afterRevision !== server.revision, server.revision) },
    commands: { place: async () => ({ kind: 'unknown' }), check: async () => ({ kind: 'unknown' }) } })
  sync.start()
  root.render(<main><MuralSyncPanel key={serial} sync={sync} /></main>)
  return { sync, scene }
}
let active = mount()
async function update(patch: Partial<MuralSnapshot>) { server = { ...server, ...patch }; await active.sync.refresh() }
const hooks = {
  read: () => ({ editor: active.sync.editor.read(), sync: active.sync.read(), posts, remaining, scene: active.scene.read(), actor: ids.user, competingKey }),
  mode(value: 'lost' | 'competing') { lost = value === 'lost'; competing = value === 'competing' },
  async finishCompeting() {
    if (!competingOriginal) throw new Error('No competing fixture action')
    // An explicit fixture event models the other tab settling its already-owned
    // original. GET itself performs no mutation and the local tab sends no POST.
    await tools.submit(competingOriginal, new AbortController().signal)
    const own = wireAction(competingOriginal); ownResults.set(competingKey, own); latestOwn = own; unresolved = null
  },
  reloadEditor() { active.sync.dispose(); serial++; active = mount() },
  async remote() { const pixels = [...server.pixels]; pixels[0] = 3; last = null; await update({ pixels, revision: server.revision + 1 }) },
  async budget(value: number) { remaining = value; await active.sync.editor.refreshTools() },
  async oversized() { const pixels = Array.from({ length: 16384 }, () => 1); last = null; await update({ pixels, revision: server.revision + 1 }) },
  async freeze() { last = null; await update({ state: 'FROZEN', revision: server.revision + 1 }) },
  logout() { client.session.replace(client.session.read().generation, null) },
}
declare global { interface Window { __WALL_B__: typeof hooks } }
window.__WALL_B__ = hooks
