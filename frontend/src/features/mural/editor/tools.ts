import { z } from 'zod'
import { pixelOperationSchema, placePixelSchema } from './contract'
import type { MuralSnapshot } from './contract'
import type { PrivacyScope } from '../../../shared/query/privacy'

// Frontend domain port, NOT a second public wire contract. The authenticated
// adapter must qualify its response before mapping the frozen C04 DTO here.
const uuid = pixelOperationSchema.shape.operation_id
const revision = pixelOperationSchema.shape.revision
const utc = pixelOperationSchema.shape.committed_at
const coordinate = placePixelSchema.shape.x
export const eraseBodySchema = z.strictObject({ expected_revision: revision, x: coordinate, y: coordinate })
export const fillBodySchema = placePixelSchema
export const undoBodySchema = z.strictObject({ expected_revision: revision, target_action_id: uuid })
export const recoveredCommandSchema = z.discriminatedUnion('operation', [
  z.strictObject({ operation: z.literal('placeConcertPixel'), key: uuid, body: placePixelSchema }),
  z.strictObject({ operation: z.literal('erase'), key: uuid, body: eraseBodySchema }),
  z.strictObject({ operation: z.literal('fill'), key: uuid, body: fillBodySchema }),
  z.strictObject({ operation: z.literal('undo'), key: uuid, body: undoBodySchema }),
])
const undoTargetSchema = z.strictObject({ actionId: uuid, revision, changedCells: z.number().int().min(1).max(256), expiresAt: utc })
export const toolContextSchema = z.strictObject({
  version: z.literal(1), actorId: uuid, eventId: uuid, revision, serverTime: utc, editable: z.boolean(), nextAllowedAt: utc.nullable(),
  capabilities: z.strictObject({ pencil: z.boolean(), erase: z.boolean(), fill: z.boolean(), undo: z.boolean(), backgroundIndex: z.literal(0).nullable() }),
  fillLimit: z.literal(256), undoWindowMs: z.literal(600000),
  budget: z.strictObject({ limit: z.literal(512), windowMs: z.literal(600000), remaining: z.number().int().min(0).max(512), nextCellAvailableAt: utc.nullable() }),
  undoTarget: undoTargetSchema.nullable(), unresolved: recoveredCommandSchema.nullable(),
})
export const actionReceiptSchema = z.strictObject({ actionId: uuid, eventId: uuid, actorId: uuid, commandKey: uuid,
  operation: z.enum(['erase', 'fill', 'undo']), revision, changedCells: z.number().int().min(1).max(256),
  committedAt: utc, nextAllowedAt: utc, resetRequired: z.literal(true),
})
export type DrawingTool = 'pencil' | 'erase' | 'fill'
export type WallToolContext = Readonly<z.infer<typeof toolContextSchema>>
export type WallActionReceipt = Readonly<z.infer<typeof actionReceiptSchema>>
export type RecoveredCommand = z.infer<typeof recoveredCommandSchema>
export type WallActionIdentity = Exclude<RecoveredCommand, { operation: 'placeConcertPixel' }>
export type WallActionCommand = WallActionIdentity & { readonly scope: PrivacyScope }
export type WallActionResult = { readonly kind: 'acknowledged'; readonly receipt: unknown }
  | { readonly kind: 'pending' | 'unknown' }
  | { readonly kind: 'rejected'; readonly reason: string }
export interface WallToolPort {
  /** Authenticated, no-store, server-owned reload recovery; no browser persistence. */
  load(scope: PrivacyScope, signal: AbortSignal): Promise<unknown>
  submit(command: WallActionCommand, signal: AbortSignal): Promise<WallActionResult>
  /** Recover exactly the original key/body. Never silently POST or mint a new key. */
  check(command: WallActionCommand, signal: AbortSignal): Promise<WallActionResult>
}
export interface WallToolTimers {
  set(ms: number, run: () => void): unknown
  clear(timer: unknown): void
}
/** Shared C04 point-in-time capability rule; monotonic request START owns expiry. */
export const WALL_TOOL_FRESH_MS = 5000
export const WALL_TOOL_REFRESH_MS = 2000
export interface WallDraft {
  readonly operation: 'erase' | 'fill' | 'undo'
  readonly revision: number
  readonly count: number
  readonly cells: readonly number[]
  readonly body: WallActionIdentity['body']
}
export type DraftResult = { readonly ok: true; readonly draft: WallDraft }
  | { readonly ok: false; readonly reason: 'NO_CHANGE' | 'FILL_TOO_LARGE' | 'INVALID_INPUT' }

/** Preview only. A bounded, four-connected component; never truncates a fill. */
export function previewWallAction(snapshot: MuralSnapshot, operation: 'erase' | 'fill', x: number, y: number, colorIndex: number): DraftResult {
  if (!placePixelSchema.safeParse({ expected_revision: snapshot.revision, x, y, color_index: colorIndex }).success) return { ok: false, reason: 'INVALID_INPUT' }
  const seed = y * 128 + x, original = snapshot.pixels[seed]
  if (original === colorIndex) return { ok: false, reason: 'NO_CHANGE' }
  const cells: number[] = []
  if (operation === 'erase') cells.push(seed)
  else {
    const visited = new Uint8Array(16384), queue = [seed]
    visited[seed] = 1
    for (let head = 0; head < queue.length; head++) {
      const cell = queue[head]!
      if (snapshot.pixels[cell] !== original) continue
      cells.push(cell)
      if (cells.length > 256) return { ok: false, reason: 'FILL_TOO_LARGE' }
      const cx = cell % 128, cy = Math.floor(cell / 128)
      const neighbors = [cx > 0 ? cell - 1 : -1, cx < 127 ? cell + 1 : -1, cy > 0 ? cell - 128 : -1, cy < 127 ? cell + 128 : -1]
      for (const next of neighbors) if (next >= 0 && !visited[next]) { visited[next] = 1; queue.push(next) }
    }
  }
  const body = operation === 'erase' ? { expected_revision: snapshot.revision, x, y }
    : { expected_revision: snapshot.revision, x, y, color_index: colorIndex }
  return { ok: true, draft: Object.freeze({ operation, revision: snapshot.revision, count: cells.length,
    cells: Object.freeze(cells), body: Object.freeze(body) }) }
}

export function matchActionReceipt(input: unknown, command: WallActionCommand): WallActionReceipt | null {
  const parsed = actionReceiptSchema.safeParse(input)
  if (!parsed.success) return null
  const r = parsed.data
  return r.eventId === command.scope.eventId && r.actorId === command.scope.userId && r.commandKey === command.key
    && r.operation === command.operation && r.revision === command.body.expected_revision + 1
    && (command.operation !== 'erase' || r.changedCells === 1)
    && Date.parse(r.nextAllowedAt) >= Date.parse(r.committedAt) ? Object.freeze(r) : null
}

// Only adapter-verified terminal server rejections are actionable. An unknown
// reason, malformed response, timeout or transport failure remains uncertain.
export const toolRejections = new Set(['INVALID_INPUT', 'AUTHENTICATION_REQUIRED', 'PERMISSION_DENIED', 'ORIGIN_NOT_ALLOWED',
  'BROWSER_INTENT_REQUIRED', 'RESOURCE_UNAVAILABLE', 'RATE_LIMITED', 'MURAL_COOLDOWN', 'VERSION_CONFLICT', 'INVALID_TRANSITION',
  'EVENT_READ_ONLY', 'MURAL_PAUSED', 'MURAL_FROZEN', 'MURAL_REGION_LOCKED', 'PAYLOAD_TOO_LARGE', 'INVALID_MEDIA_TYPE',
  'MURAL_NO_CHANGE', 'MURAL_FILL_TOO_LARGE', 'MURAL_BUDGET_EXCEEDED', 'MURAL_UNDO_EXPIRED', 'MURAL_UNDO_NOT_LATEST', 'MURAL_UNDO_UNAVAILABLE', 'MURAL_TOOL_UNAVAILABLE',
  // The HTTP adapter emits this as rejected ONLY for a bound FAILED receipt.
  'MURAL_ACTION_UNRESOLVED'])
