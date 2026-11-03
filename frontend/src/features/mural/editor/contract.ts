import { z } from 'zod'
import type { PrivacyScope, createQueryPrivacy } from '../../../shared/query/privacy'

const uuid = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
const revision = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
const utc = z.string().refine(v => /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?Z$/.test(v)
  && Number.isFinite(Date.parse(v)) && new Date(v).toISOString().slice(0, 19) === v.slice(0, 19))
const coordinate = z.number().int().min(0).max(127)
const color = z.number().int().min(0).max(15)
// Locally owned C04 DTO boundary; schemas/contracts remain delivery-owned.
export const placePixelSchema = z.strictObject({ expected_revision: revision, x: coordinate, y: coordinate, color_index: color })
export const snapshotSchema = z.strictObject({ snapshot_id: uuid, event_id: uuid, revision,
  state: z.enum(['EDITABLE', 'PAUSED', 'FROZEN']), width: z.literal(128), height: z.literal(128),
  palette: z.array(z.string().regex(/^#[0-9A-F]{6}$/)).length(16), pixels: z.array(color).length(16384),
  created_at: utc, server_time: utc, cooldown_ms: z.number().int().min(0).max(3600000), next_allowed_at: utc.nullable() })
export const pixelOperationSchema = z.strictObject({ operation_id: uuid, event_id: uuid, revision,
  x: coordinate, y: coordinate, color_index: color, committed_at: utc, next_allowed_at: utc })
export type PlacePixel = z.infer<typeof placePixelSchema>
export type MuralSnapshot = z.infer<typeof snapshotSchema>
export type PixelOperation = z.infer<typeof pixelOperationSchema>
export type PrivacyPort = Pick<ReturnType<typeof createQueryPrivacy>, 'capture' | 'isCurrent' | 'registerCleanup'>
export interface PixelCommand {
  readonly scope: PrivacyScope
  readonly operation: 'placeConcertPixel'
  readonly key: string
  readonly body: Readonly<PlacePixel>
}
export type CommandResult =
  | { readonly kind: 'acknowledged'; readonly pixel: unknown }
  | { readonly kind: 'pending' | 'unknown' }
  // Only a verified, terminal server rejection. Transport exceptions/5xx are UNKNOWN.
  | { readonly kind: 'rejected'; readonly reason: string }
export interface MuralCommandPort {
  place(command: PixelCommand, signal: AbortSignal): Promise<CommandResult>
  /** Read-only status/result recovery for this exact actor/event/key/body; NEVER replays POST. */
  check(command: PixelCommand, signal: AbortSignal): Promise<CommandResult>
}
export type Availability = 'loading' | 'writable' | 'read-only' | 'error'
