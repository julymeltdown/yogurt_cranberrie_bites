import { QueryClient } from '@tanstack/react-query'
import schema from '../../../../../contracts/assets/v2/reviewed/avatar-manifest.schema.json'
import { createHttpClient, createMemorySession } from '../../../shared/api/http'
import { createQueryPrivacy, type PrivacyScope } from '../../../shared/query/privacy'
import { createReviewedProductionAuthority, type PublicReviewedConfig } from '../../../entities/emo-avatar/reviewed-production'
import { REVIEWED_MOTION } from '../../../entities/emo-avatar/reviewed-manifest'
import { decodeReviewedMotionConfig } from '../../../entities/emo-avatar/reviewed-config'
import { PROFILE_AVATAR_PATH } from '../api/persistence'

/** The pinned reviewed manifest document, exactly as the profile shell reads it. */
export const manifest = schema.$defs.ReviewedMotionAvatarManifest.const
export const pin = { manifest_id: REVIEWED_MOTION.manifestId, manifest_version: 2, content_hash: REVIEWED_MOTION.contentHash }
export const preset = (index: number) => decodeReviewedMotionConfig(manifest.presets[index]!.config)
export const layered = (body = 'motion_v2_body_0') => decodeReviewedMotionConfig({ contract_version: '2.0.0', kind: 'avatar_config',
  manifest_id: pin.manifest_id, manifest_version: 2, representation: 'motion_layered',
  item_ids: { body, hair: 'motion_v2_hair_0', eyes: 'motion_v2_eyes_0', outfit: null, accessory: null } })

export function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(yes => { resolve = yes })
  return { promise, resolve }
}

export const ACCOUNT = '00000000-0000-4000-8000-000000000811'
export const OTHER_ACCOUNT = '00000000-0000-4000-8000-000000000812'
const EVENT = '00000000-0000-4000-8000-000000000701'

export type RecordedCall = { path: string; method: string; body: unknown }

/**
 * A profile-avatar fixture.
 *
 * It composes the *real* memory session, privacy coordinator and HTTP client,
 * and stubs only `fetch`. That means every assertion about the request path is
 * an assertion about what the production adapter would actually send.
 */
export async function setup(options: { readonly userId?: string } = {}) {
  const session = createMemorySession(), client = new QueryClient(), privacy = createQueryPrivacy({ session, client })
  session.replace(0, { userId: options.userId ?? ACCOUNT, eventId: EVENT, accessToken: 'deterministic-test-token' })
  const scope = privacy.capture(), calls: RecordedCall[] = []
  let avatar: { revision: number; config: PublicReviewedConfig } = { revision: 1, config: preset(0) }
  let mode: 'ok' | 'conflict' | 'uncertain' | 'denied' | 'bad-revision' | 'stale-context' = 'ok'
  let handler: ((request: Request) => Response | Promise<Response>) | null = null
  const http = createHttpClient({ session, fetch: async input => {
    if (!(input instanceof Request)) throw Error('Expected canonical Request')
    const path = new URL(input.url).pathname, body: unknown = input.method === 'PUT' ? await input.clone().json() : null
    calls.push({ path, method: input.method, body })
    if (handler) return handler(input)
    if (path !== PROFILE_AVATAR_PATH) throw Error(`Unexpected profile avatar request: ${path}`)
    if (input.method === 'GET') return Response.json(structuredClone(avatar))
    if (mode === 'denied') return Response.json({ code: 'DENIED', message: 'Denied', details: { reason: 'PERMISSION_DENIED' } }, { status: 403 })
    if (mode === 'stale-context') return Response.json({ code: 'STALE', message: 'Stale', details: { reason: 'ACCOUNT_CONTEXT_STALE' } }, { status: 409 })
    const request = body as Record<string, unknown>
    if (mode === 'conflict' || request.expected_revision !== avatar.revision) {
      // The frozen core error envelope is closed: `current_revision` is the only
      // extra field a VERSION_CONFLICT may carry, and the client re-reads for the
      // current config.
      return Response.json({ code: 'CONFLICT', message: 'Changed', details: { reason: 'VERSION_CONFLICT', current_revision: avatar.revision } }, { status: 409 })
    }
    avatar = { revision: avatar.revision + 1, config: decodeReviewedMotionConfig(request.config) }
    if (mode === 'uncertain') throw Error('Response lost after commit')
    return Response.json({ ...structuredClone(avatar), revision: mode === 'bad-revision' ? avatar.revision + 1 : avatar.revision })
  } })
  const auth = { session, privacy, request: http.request }, signal = new AbortController()
  // The scope is captured at creation time, so an authority minted after an
  // account switch is bound to the *new* account, exactly as production does.
  const current = () => privacy.capture()
  return {
    auth, scope, signal, calls, pin, session, privacy,
    create: () => createReviewedProductionAuthority({ auth, scope: current(), pin, manifest, canSave: true, signal: signal.signal }),
    server: () => avatar,
    setServer: (next: typeof avatar) => { avatar = next },
    mode: (next: typeof mode) => { mode = next },
    handler: (next: typeof handler) => { handler = next },
    /** Switch the canonical account, exactly as a real login switch does. */
    switchAccount(userId: string) {
      const expected = session.read()
      session.replace(expected.generation, { userId, eventId: EVENT, accessToken: 'deterministic-test-token' })
      return privacy.capture()
    },
    dispose() { signal.abort(); privacy.dispose(); client.clear() },
  }
}

export type Fixture = Awaited<ReturnType<typeof setup>>
export type { PrivacyScope }
