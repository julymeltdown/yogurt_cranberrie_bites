/**
 * WP-07 test fixtures: a room server that speaks the five frozen operations.
 *
 * The fixture composes the *real* `createRoomClient` and the real decoders, and
 * stubs only the transport, so every assertion about a request path or body is an
 * assertion about what the production client would actually send.
 */
import type { HttpRequest } from '../../../shared/api/http'
import { createRoomClient, type OwnRoom, type ProfileRoom, type RoomClient, type RoomTransport, type RoomVisibility } from '../api/room'
import { initialLayout, type RoomLayout } from '../model/layout'
import type { PosterConverter } from '../worker/client'
import type { Crop, PosterPreset, PosterPixels } from '../pixelize'

export const OWNER = '00000000-0000-4000-8000-0000000007a1'
export const VISITOR = '00000000-0000-4000-8000-0000000007a2'
export const POSTER_MEDIA = '11111111-1111-4111-8111-111111111111'

/** The session scope the fixture transport reports; the room client ignores it. */
const SCOPE = Object.freeze({ generation: 0, userId: OWNER, eventId: null })

export interface RecordedCall {
  readonly path: string
  readonly method: string
  readonly body: unknown
}

export interface RoomServer {
  readonly client: RoomClient
  readonly calls: RecordedCall[]
  readonly read: () => { layout: RoomLayout; revision: number; visibility: RoomVisibility }
  readonly set: (layout: RoomLayout, revision?: number, visibility?: RoomVisibility) => void
  readonly mode: (mode: Mode) => void
  readonly setOwn: (owner: boolean) => void
}

export type Mode = 'ok' | 'conflict' | 'conflict-with-room' | 'unavailable' | 'failed' | 'save-failed' | 'upload-failed'

/** The wire shape of an own room, exactly as the server projects it. */
export function ownRoomWire(layout: RoomLayout, revision: number, visibility: RoomVisibility, owner = OWNER) {
  return {
    owner_profile_id: owner,
    is_owner: true as const,
    visibility,
    layout,
    posters: [],
    poster_moderation: [],
    revision,
    updated_at: '2026-11-09T00:00:00.000Z',
  }
}

export function profileRoomWire(layout: RoomLayout, revision: number, visibility: RoomVisibility, owner = OWNER) {
  return {
    owner_profile_id: owner,
    visibility,
    layout,
    posters: [],
    revision,
    updated_at: '2026-11-09T00:00:00.000Z',
  }
}

export function createRoomServer(): RoomServer {
  let layout = initialLayout()
  let revision = 1
  let visibility: RoomVisibility = 'PUBLIC'
  let mode: Mode = 'ok'
  const calls: RecordedCall[] = []
  let posterCounter = 0

  const transport: RoomTransport = {
    async request<T>(request: HttpRequest<T>) {
      const body = request.body && 'json' in request.body ? request.body.json : null
      calls.push({ path: request.path, method: request.method, body })
      if (mode === 'failed') throw Object.assign(new Error('network'), { kind: 'network' })
      if (mode === 'save-failed' && request.method !== 'GET') {
        throw Object.assign(new Error('unavailable'), { kind: 'http', status: 503, server: { envelope: { code: 'ROOM_503', message: 'Room request unavailable', details: { reason: 'DEPENDENCY_UNAVAILABLE' } }, core: { reason: 'DEPENDENCY_UNAVAILABLE' }, reason: 'DEPENDENCY_UNAVAILABLE' } })
      }
      if (mode === 'unavailable') {
        throw Object.assign(new Error('unavailable'), {
          kind: 'http', status: 404,
          server: { envelope: { code: 'ROOM_404', message: 'Room request unavailable', details: { reason: 'RESOURCE_UNAVAILABLE' } }, core: { reason: 'RESOURCE_UNAVAILABLE' }, reason: 'RESOURCE_UNAVAILABLE' },
        })
      }
      if (request.method === 'GET' && request.path === '/api/me/room') {
        return { data: request.decode(ownRoomWire(layout, revision, visibility), { scope: SCOPE, status: 200, path: request.path }), scope: {} } as never
      }
      if (request.method === 'GET' && request.path.startsWith('/api/profiles/')) {
        return { data: request.decode(profileRoomWire(layout, revision, visibility), { scope: SCOPE, status: 200, path: request.path }), scope: {} } as never
      }
      if (request.method === 'PUT' && request.path === '/api/me/room') {
        const submitted = body as { expected_revision: number; layout: RoomLayout }
        if (mode === 'conflict' || mode === 'conflict-with-room' || submitted.expected_revision !== revision) {
          const envelope = mode === 'conflict-with-room'
            ? { code: 'ROOM_409', message: 'Room request unavailable', details: { reason: 'VERSION_CONFLICT', current: ownRoomWire(layout, revision, visibility) } }
            : { code: 'ROOM_409', message: 'Room request unavailable', details: { reason: 'VERSION_CONFLICT' } }
          throw Object.assign(new Error('conflict'), { kind: 'http', status: 409, server: { envelope, core: { reason: 'VERSION_CONFLICT' }, reason: 'VERSION_CONFLICT' } })
        }
        layout = submitted.layout
        revision += 1
        return { data: request.decode(ownRoomWire(layout, revision, visibility), { scope: SCOPE, status: 200, path: request.path }), scope: {} } as never
      }
      if (request.method === 'PATCH' && request.path === '/api/me/room/visibility') {
        const submitted = body as { expected_revision: number; visibility: RoomVisibility }
        if (submitted.expected_revision !== revision) {
          throw Object.assign(new Error('conflict'), { kind: 'http', status: 409, server: { envelope: { code: 'ROOM_409', message: 'Room request unavailable', details: { reason: 'VERSION_CONFLICT' } }, core: { reason: 'VERSION_CONFLICT' }, reason: 'VERSION_CONFLICT' } })
        }
        visibility = submitted.visibility
        revision += 1
        return { data: request.decode(ownRoomWire(layout, revision, visibility), { scope: SCOPE, status: 200, path: request.path }), scope: {} } as never
      }
      if (request.method === 'POST' && request.path === '/api/me/room/posters') {
        if (mode === 'upload-failed') {
          throw Object.assign(new Error('invalid'), { kind: 'http', status: 400, server: { envelope: { code: 'ROOM_400', message: 'Room request unavailable', details: { reason: 'INVALID_INPUT' } }, core: { reason: 'INVALID_INPUT' }, reason: 'INVALID_INPUT' } })
        }
        const submitted = body as { preset: PosterPreset; width: number; height: number }
        posterCounter += 1
        const mediaId = `22222222-2222-4222-8222-${String(posterCounter).padStart(12, '0')}`
        return {
          data: request.decode({
            media: { media_id: mediaId, state: 'READY', revision: 1, width: submitted.width, height: submitted.height, content_type: 'image/png', purpose: 'POST', read_path: `/api/profiles/room/media/${mediaId}/content` },
            preset: submitted.preset, width: submitted.width, height: submitted.height, revision: 1,
          }, { scope: SCOPE, status: 200, path: request.path }),
          scope: {},
        } as never
      }
      throw Object.assign(new Error('unexpected'), { kind: 'network' })
    },
  }

  return {
    client: createRoomClient(transport),
    calls,
    read: () => ({ layout, revision, visibility }),
    set: (next, nextRevision = revision, nextVisibility = visibility) => {
      layout = next
      revision = nextRevision
      visibility = nextVisibility
    },
    mode: next => { mode = next },
    setOwn: () => undefined,
  }
}

/** The fixture `OwnRoom` a screen receives. */
export type FixtureOwnRoom = OwnRoom
export type FixtureProfileRoom = ProfileRoom

/** A conversion port that returns deterministic pixels without a worker. */
export function createFixtureConverter(): PosterConverter & { readonly calls: number } {
  const state = { calls: 0 }
  return {
    get calls() { return state.calls },
    async convert(_source: Blob, crop: Crop, preset: PosterPreset) {
      state.calls += 1
      const side = preset === 'FINE' ? 64 : 32
      const pixels: PosterPixels = { preset, side, rgba: new Uint8ClampedArray(side * side * 4).fill(128) }
      void crop
      return { blob: new Blob([`PNG:${preset}:${side}`], { type: 'image/png' }), side: pixels.side }
    },
    dispose() {},
  }
}
