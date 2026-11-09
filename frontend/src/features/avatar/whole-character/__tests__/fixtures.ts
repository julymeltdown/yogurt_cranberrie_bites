import { QueryClient } from '@tanstack/react-query'
import participant from '../../../../../../contracts/fixtures/core/private-participant.json'
import manifest from '../../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import conflict from '../../../../../../contracts/fixtures/core/error-version-conflict.json'
import { createMemorySession, createHttpClient } from '../../../../shared/api/http'
import { createQueryPrivacy } from '../../../../shared/query/privacy'
import { createWholeCharacterEditorController, type WholeCharacterChoice } from '../controller'
import { decodeWholeConfig, type WholeCharacterState } from '../model'
import { deferred } from '../../../../test-support/fixtures'
export { deferred }

export const configs = manifest.presets.map(p => decodeWholeConfig(p.config))
export const initial: WholeCharacterState = Object.freeze({ revision: 1, config: configs[0]! })
type Mode = 'ok' | 'denied' | 'conflict' | 'uncertain' | 'invalid' | 'slow' | 'other-user' | 'read-failed'
/** Test-only canonical HTTP/Privacy harness, no synthetic character drawing or production adapter. */
export function harness() {
  const session = createMemorySession(), query = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  session.replace(0, { userId: participant.user_id, eventId: participant.event_id, accessToken: 'whole-fixture-token' })
  const privacy = createQueryPrivacy({ client: query, session }), scope = privacy.capture()
  let mode: Mode = 'ok', server = structuredClone(initial), runtimeDisposed = 0, choiceCalls = 0
  const requests: { method: string; body: unknown; headers: Headers }[] = [], commits: WholeCharacterState[] = []
  const pending = deferred<Response>(), choices: readonly WholeCharacterChoice[] = Object.freeze(configs.map((config, i) => Object.freeze({ id: config.preset_id, label: `Fixture character ${i + 1}`, config })))
  const runtime = {
    choices: async (_signal: AbortSignal) => { choiceCalls++; return choices },
    isCurrent: () => privacy.isCurrent(scope),
    dispose: () => { runtimeDisposed++ },
  }
  const http = createHttpClient({ session, fetch: async (input, init) => {
    const request = input instanceof Request ? input : new Request(input, init)
    const body = request.method === 'PUT' ? await request.clone().json() : null
    requests.push({ method: request.method, body, headers: request.headers })
    if (request.method === 'GET') {
      if (mode === 'read-failed') return Response.json({}, { status: 503 })
      return Response.json({ ...participant, revision: 101, user_id: mode === 'other-user' ? '00000000-0000-4000-8000-000000000099' : participant.user_id,
        profile: { ...participant.profile, avatar: server } })
    }
    if (mode === 'slow') return pending.promise
    if (mode === 'denied') return Response.json({ code: 'GATEWAY_403', message: 'Denied', details: { reason: 'PERMISSION_DENIED' } }, { status: 403 })
    if (mode === 'conflict' || body.expected_revision !== server.revision) return Response.json(conflict, { status: 409 })
    server = { revision: server.revision + 1, config: body.config }
    if (mode === 'uncertain') return Response.json({ code: 'UPSTREAM', message: 'Unknown after commit', details: { reason: 'DEPENDENCY_UNAVAILABLE' } }, { status: 503 })
    if (mode === 'invalid') return Response.json({ revision: server.revision, config: configs[5] })
    return Response.json(server)
  } })
  const controller = createWholeCharacterEditorController({ auth: { privacy, request: http.request }, scope, runtime,
    onCommitted: value => commits.push(value) })
  return { controller, session, privacy, query, runtime, scope, requests, commits, pending,
    setMode(value: Mode) { mode = value }, setServer(value: WholeCharacterState) { server = structuredClone(value) },
    stats: () => ({ runtimeDisposed, choiceCalls }),
    dispose() { controller.dispose(); privacy.dispose(); query.clear() },
  }
}
