/**
 * WP-07 T-602..T-605: the room API client.
 *
 * Five operations, exactly the frozen ones of http-contract.md §2.4:
 *
 * | operationId | Method | Path |
 * | --- | --- | --- |
 * | `getProfileRoom` | GET | `/api/profiles/{profileId}/room` |
 * | `getOwnRoom` | GET | `/api/me/room` |
 * | `saveOwnRoom` | PUT | `/api/me/room` |
 * | `setOwnRoomVisibility` | PATCH | `/api/me/room/visibility` |
 * | `uploadRoomPoster` | POST | `/api/me/room/posters` |
 *
 * A revision conflict is `409 VERSION_CONFLICT` and the server returns the
 * current room in `details.current`, so the client can offer an explicit
 * resolution without a second round trip. The client never decides visibility: it
 * renders what the server returned and shows a neutral unavailable state for a
 * `404`.
 */
import { z } from 'zod'
import type { HttpRequest } from '../../../shared/api/http'
import type { TransportError } from '../../../shared/api/errors'
import { decodeLayout, type RoomLayout } from '../model/layout'

/** The five frozen paths. There is no event segment and no intro segment. */
export const ROOM_PATHS = Object.freeze({
  own: '/api/me/room',
  visibility: '/api/me/room/visibility',
  posters: '/api/me/room/posters',
  profile: (profileId: string) => `/api/profiles/${profileId}/room`,
})

const uuid = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
const revision = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
const timestamp = z.string().regex(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/)

/** `Domain_feed_Media`, as the room projects a poster. */
const posterMediaSchema = z.strictObject({
  media_id: uuid,
  state: z.literal('READY'),
  revision,
  width: z.number().int().min(1).max(4096),
  height: z.number().int().min(1).max(4096),
  content_type: z.literal('image/png'),
  purpose: z.string().min(1).max(16),
  read_path: z.string().min(1).max(512),
})
export type PosterMedia = z.infer<typeof posterMediaSchema>

const posterModerationSchema = z.strictObject({
  media_id: uuid,
  state: z.enum(['VISIBLE', 'HIDDEN', 'PENDING_REVIEW']),
})

const visibilitySchema = z.enum(['PUBLIC', 'PRIVATE'])
export type RoomVisibility = z.infer<typeof visibilitySchema>

const profileRoomSchema = z.strictObject({
  owner_profile_id: uuid,
  visibility: visibilitySchema,
  layout: z.unknown(),
  posters: z.array(posterMediaSchema).max(4),
  revision,
  updated_at: timestamp,
})

const ownRoomSchema = profileRoomSchema.extend({
  is_owner: z.literal(true),
  poster_moderation: z.array(posterModerationSchema).max(4),
})

export interface ProfileRoom {
  readonly ownerProfileId: string
  readonly visibility: RoomVisibility
  readonly layout: RoomLayout
  readonly posters: readonly PosterMedia[]
  readonly revision: number
  readonly updatedAt: string
}

export interface OwnRoom extends ProfileRoom {
  readonly isOwner: true
  readonly posterModeration: readonly z.infer<typeof posterModerationSchema>[]
}

const freeze = <T extends object>(value: T): Readonly<T> => Object.freeze(value)

/** Decode a `social.ProfileRoom`. Throws on anything else. */
export function decodeProfileRoom(value: unknown): ProfileRoom {
  const row = profileRoomSchema.parse(value)
  return freeze({
    ownerProfileId: row.owner_profile_id,
    visibility: row.visibility,
    layout: decodeLayout(row.layout),
    posters: freeze(row.posters.map(poster => freeze(poster))),
    revision: row.revision,
    updatedAt: row.updated_at,
  })
}

/** Decode a `social.OwnRoom`. Throws on anything else. */
export function decodeOwnRoom(value: unknown): OwnRoom {
  const row = ownRoomSchema.parse(value)
  return freeze({
    ownerProfileId: row.owner_profile_id,
    visibility: row.visibility,
    layout: decodeLayout(row.layout),
    posters: freeze(row.posters.map(poster => freeze(poster))),
    revision: row.revision,
    updatedAt: row.updated_at,
    isOwner: true as const,
    posterModeration: freeze(row.poster_moderation.map(entry => freeze(entry))),
  })
}

/** `social.RoomPoster`, the answer to an upload. */
const roomPosterSchema = z.strictObject({
  media: posterMediaSchema,
  preset: z.enum(['FINE', 'CHUNKY']),
  width: z.number().int().min(8).max(128),
  height: z.number().int().min(8).max(128),
  revision,
})
export type RoomPoster = z.infer<typeof roomPosterSchema>
export const decodeRoomPoster = (value: unknown): RoomPoster => freeze(roomPosterSchema.parse(value))

/**
 * The five frozen requests.
 *
 * Each is a concrete `HttpRequest<T>`, not a union member: the transport is
 * generic over the decoded type, so a union would force every call site to widen
 * to the narrowest member. The request builders are the only place a room path,
 * method, body shape or decoder is written.
 */
export function ownRoomRequest(): HttpRequest<OwnRoom> {
  return { path: ROOM_PATHS.own, method: 'GET', access: 'protected', errorFamily: 'core', decode: decodeOwnRoom }
}

export function profileRoomRequest(profileId: string): HttpRequest<ProfileRoom> {
  if (!uuid.safeParse(profileId).success) throw new Error('Invalid profile id')
  return { path: ROOM_PATHS.profile(profileId), method: 'GET', access: 'protected', errorFamily: 'core', decode: decodeProfileRoom }
}

export function saveRoomRequest(expectedRevision: number, layout: RoomLayout): HttpRequest<OwnRoom> {
  if (!revision.safeParse(expectedRevision).success) throw new Error('Invalid revision')
  return {
    path: ROOM_PATHS.own,
    method: 'PUT',
    access: 'protected',
    errorFamily: 'core',
    body: { json: { expected_revision: expectedRevision, layout } },
    decode: decodeOwnRoom,
  }
}

export function setVisibilityRequest(expectedRevision: number, visibility: RoomVisibility): HttpRequest<OwnRoom> {
  if (!revision.safeParse(expectedRevision).success) throw new Error('Invalid revision')
  return {
    path: ROOM_PATHS.visibility,
    method: 'PATCH',
    access: 'protected',
    errorFamily: 'core',
    body: { json: { expected_revision: expectedRevision, visibility } },
    decode: decodeOwnRoom,
  }
}

export function uploadPosterRequest(input: {
  readonly preset: 'FINE' | 'CHUNKY'
  readonly width: number
  readonly height: number
  readonly bytesBase64: string
}): HttpRequest<RoomPoster> {
  return {
    path: ROOM_PATHS.posters,
    method: 'POST',
    access: 'protected',
    errorFamily: 'core',
    body: { json: { preset: input.preset, width: input.width, height: input.height, content_type: 'image/png', bytes_base64: input.bytesBase64 } },
    decode: decodeRoomPoster,
  }
}

/**
 * The current room a `409 VERSION_CONFLICT` carries.
 *
 * The frozen core envelope allows `details.current_revision` and nothing else, so
 * the current *layout* is not in the error body; the caller re-reads the room to
 * obtain it. This helper extracts the revision the conflict named when the server
 * supplied one, so the caller can decide whether a re-read is even necessary.
 */
export function conflictRevision(error: TransportError): number | null {
  if (error.kind !== 'http' || error.status !== 409) return null
  const details = error.server?.core
  return details && details.reason === 'VERSION_CONFLICT' && 'current_revision' in details
    ? (details.current_revision ?? null)
    : null
}

/**
 * The current room a `409` carries when the producer sends the whole room.
 *
 * The room producer answers a conflict with `details.current` (the current
 * `social.OwnRoom`), which is what lets the editor offer an explicit resolution
 * without a second round trip. The shared core error parser is closed to
 * `current_revision`, so the room client reads the body itself through a
 * dedicated decoder rather than widening the core envelope for every operation.
 */
export function decodeConflictRoom(value: unknown): OwnRoom | null {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return null
  const details = (value as { details?: unknown }).details
  if (details === null || typeof details !== 'object' || Array.isArray(details)) return null
  const current = (details as { current?: unknown }).current
  if (current === undefined) return null
  return decodeOwnRoom(current)
}

/**
 * The one transport call the room client makes.
 *
 * The caller injects the transport (the shell's HTTP client, or a fixture), so
 * this module owns the request shapes and the response decoding but never the
 * session, the credentials or the retry policy.
 */
export interface RoomTransport {
  /** The shell's transport: one generic request, decoded by the request itself. */
  request<T>(request: HttpRequest<T>): Promise<{ readonly data: T; readonly scope: unknown }>
}

/** A conflict as the editor needs it: the current room, already decoded. */
export class RoomConflictError extends Error {
  constructor(readonly current: OwnRoom | null) {
    super('The room changed while you were editing.')
    this.name = 'RoomConflictError'
  }
}

/** A room the viewer may not open, or one that does not exist. */
export class RoomUnavailableError extends Error {
  constructor() {
    super('This room is unavailable.')
    this.name = 'RoomUnavailableError'
  }
}

/** The client the screens use. Every method binds the frozen paths and decoders. */
export function createRoomClient(transport: RoomTransport) {
  // One call per operation, each with its own concrete request type: the
  // transport's generic is instantiated at the call site, never widened.
  async function own(): Promise<OwnRoom> {
    return guard(transport.request<OwnRoom>(ownRoomRequest()))
  }
  async function profile(profileId: string): Promise<ProfileRoom> {
    return guard(transport.request<ProfileRoom>(profileRoomRequest(profileId)))
  }
  async function save(expectedRevision: number, layout: RoomLayout): Promise<OwnRoom> {
    return guard(transport.request<OwnRoom>(saveRoomRequest(expectedRevision, layout)))
  }
  async function visibility(expectedRevision: number, value: RoomVisibility): Promise<OwnRoom> {
    return guard(transport.request<OwnRoom>(setVisibilityRequest(expectedRevision, value)))
  }
  async function poster(input: { readonly preset: 'FINE' | 'CHUNKY'; readonly width: number; readonly height: number; readonly bytesBase64: string }): Promise<RoomPoster> {
    return guard(transport.request<RoomPoster>(uploadPosterRequest(input)))
  }
  async function guard<T>(pending: Promise<{ readonly data: T; readonly scope: unknown }>): Promise<T> {
    try {
      const result = await pending
      return result.data
    } catch (error) {
      const failure = error as TransportError
      if (failure && typeof failure === 'object' && 'kind' in failure) {
        if (failure.kind === 'http' && failure.status === 409) {
          throw new RoomConflictError(conflictRoomOf(failure))
        }
        if (isRoomUnavailable(failure)) throw new RoomUnavailableError()
      }
      throw error
    }
  }

  return Object.freeze({
    readOwn: () => own(),
    readProfile: (profileId: string) => profile(profileId),
    save,
    setVisibility: visibility,
    uploadPoster: poster,
  })
}

export type RoomClient = ReturnType<typeof createRoomClient>

/**
 * The decoded current room of a `409`, when the transport kept the raw body.
 *
 * A transport that parses the frozen core envelope reports the conflict without
 * the room (the envelope is closed to `current_revision`); in that case the caller
 * re-reads the room. A transport that also kept the raw body lets the editor offer
 * the resolution immediately. Both paths end in the same explicit choice.
 */
function conflictRoomOf(error: TransportError): OwnRoom | null {
  const raw = (error as TransportError & { readonly rawBody?: unknown }).rawBody
  if (raw === undefined) return null
  try {
    return decodeConflictRoom(raw)
  } catch {
    return null
  }
}

/** Whether an error is the `404` of a room a visitor may not open. */
export function isRoomUnavailable(error: TransportError): boolean {
  return error.kind === 'http' && error.status === 404
    && error.server?.core?.reason === 'RESOURCE_UNAVAILABLE'
}
