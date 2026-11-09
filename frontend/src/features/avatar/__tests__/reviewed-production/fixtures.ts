import { QueryClient } from '@tanstack/react-query'
import bootstrap from '../../../../../../contracts/fixtures/core/bootstrap-open.json'
import schema from '../../../../../../contracts/assets/v2/reviewed/avatar-manifest.schema.json'
import { canonicalJson, object } from '../../../auth/model/contract'
import { createHttpClient, createMemorySession } from '../../../../shared/api/http'
import { createQueryPrivacy } from '../../../../shared/query/privacy'
import { createReviewedProductionAuthority, type PublicReviewedConfig } from '../../../../entities/avatar/runtime/motion-adoption/reviewed-production'
import { REVIEWED_MOTION } from '../../../../entities/avatar/runtime/motion-adoption/reviewed-manifest'
import { decodeReviewedMotionConfig } from '../../../../entities/avatar/runtime/motion-adoption/reviewed-config'
export { deferred } from '../../../../test-support/fixtures'

export const manifest = schema.$defs.ReviewedMotionAvatarManifest.const
export const pin = { manifest_id: REVIEWED_MOTION.manifestId, manifest_version: 2, content_hash: REVIEWED_MOTION.contentHash }
export const preset = (index: number) => decodeReviewedMotionConfig(manifest.presets[index]!.config)
export const layered = () => decodeReviewedMotionConfig({ contract_version: '2.0.0', kind: 'avatar_config',
  manifest_id: pin.manifest_id, manifest_version: 2, representation: 'motion_layered',
  item_ids: { body: 'motion_v2_body_0', hair: 'motion_v2_hair_0', eyes: 'motion_v2_eyes_0', outfit: null, accessory: null } })
export async function rehash(document: Record<string, unknown>) {
  const { content_hash: _, ...body } = document
  const bytes = new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(canonicalJson(body))))
  document.content_hash = `sha256:${Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('')}`
}
export async function reviewedBundle() {
  const bundle = object(structuredClone(bootstrap))
  bundle.avatar_manifest = structuredClone(manifest)
  bundle.avatar = { revision: 1, config: preset(0) }
  object(object(bundle.participant).profile).avatar = structuredClone(bundle.avatar)
  object(bundle.capabilities).mutations = { available: true, reason: null }
  if (!Array.isArray(bundle.theme_manifests)) throw Error('Missing themes')
  for (const value of bundle.theme_manifests) {
    const theme = object(value)
    Object.assign(theme, { avatar_manifest_id: pin.manifest_id, avatar_manifest_version: 2, avatar_content_hash: pin.content_hash })
    await rehash(theme)
  }
  return bundle
}
export async function setup() {
  const session = createMemorySession(), client = new QueryClient(), privacy = createQueryPrivacy({ session, client })
  session.replace(0, { userId: bootstrap.participant.user_id, eventId: bootstrap.event.event_id, accessToken: 'deterministic-test-token' })
  const scope = privacy.capture(), calls: { path: string; method: string; body: unknown }[] = []
  let bundle = await reviewedBundle(), avatar: { revision: number; config: PublicReviewedConfig } = { revision: 1, config: preset(0) }
  let handler: ((request: Request) => Response | Promise<Response>) | null = null
  let mode: 'ok' | 'conflict' | 'uncertain' | 'denied' | 'bad-revision' = 'ok'
  const http = createHttpClient({ session, fetch: async input => {
    if (!(input instanceof Request)) throw Error('Expected canonical Request')
    const path = new URL(input.url).pathname, body: unknown = input.method === 'PUT' ? await input.clone().json() : null
    calls.push({ path, method: input.method, body })
    if (handler) return handler(input)
    if (path.endsWith('/bootstrap')) return Response.json(bundle)
    if (input.method === 'GET' && path.endsWith('/me')) {
      const participant = object(structuredClone(bundle.participant))
      object(participant.profile).avatar = structuredClone(avatar)
      return Response.json(participant)
    }
    if (input.method === 'PUT' && path.endsWith('/me/avatar')) {
      if (mode === 'denied') return Response.json({ code: 'DENIED', message: 'Denied', details: { reason: 'PERMISSION_DENIED' } }, { status: 403 })
      const request = object(body)
      if (mode === 'conflict' || request.expected_revision !== avatar.revision) return Response.json({ code: 'CONFLICT', message: 'Changed', details: { reason: 'VERSION_CONFLICT' } }, { status: 409 })
      avatar = { revision: avatar.revision + 1, config: decodeReviewedMotionConfig(request.config) }
      if (mode === 'uncertain') throw Error('Response lost after commit')
      return Response.json({ ...avatar, revision: mode === 'bad-revision' ? avatar.revision + 1 : avatar.revision })
    }
    throw Error('Unexpected request')
  } })
  const auth = { session, privacy, request: http.request }, signal = new AbortController()
  return { auth, scope, signal, calls, pin,
    create: () => createReviewedProductionAuthority({ auth, scope, pin, signal: signal.signal }),
    bundle: () => bundle, setBundle: (next: typeof bundle) => { bundle = next },
    server: () => avatar, setServer: (next: typeof avatar) => { avatar = next },
    mode: (next: typeof mode) => { mode = next }, handler: (next: typeof handler) => { handler = next },
    dispose() { signal.abort(); privacy.dispose(); client.clear() },
  }
}
