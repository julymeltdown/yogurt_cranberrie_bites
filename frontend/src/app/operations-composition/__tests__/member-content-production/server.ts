// Controlled frozen-contract HTTP, never a production role/data fallback.
import bootstrap from '../../../../../../contracts/fixtures/core/bootstrap-open.json' with { type: 'json' }
import { createReportsServer, wire, id } from '../reports-production/server.ts'
import type { Content } from '../../../../features/admin/reports/model/contract'
export { wire, id }
export const memberIds = [id(601), id(602)]
export const contentKinds = ['POST', 'COMMENT', 'PARTY', 'GIVEAWAY', 'INTRO'] as const
export function createMemberContentServer() {
  const base = createReportsServer()
  const members = new Map(memberIds.map((user_id, i) => [user_id, { user_id,
    profile: { ...structuredClone(bootstrap.participant.profile), user_id, display_name: `Review member ${i + 1}`, bio: 'PRIVATE_PROFILE_BIO' },
    state: 'ACTIVE', revision: 1 }]))
  const content: Content[] = contentKinds.map((target_type, i) => ({ target_type, target_id: id(701 + i), state: 'VISIBLE', revision: 1 }))
  type Command = { key: string; operation: string; input: Record<string, unknown>; target: string; state: 'PENDING' | 'SUCCEEDED' | 'FAILED' }
  const commands = new Map<string, Command>()
  const state = { mode: 'success' as 'success' | 'pending' | 'lost' | 'conflict' | 'denied' | 'unavailable',
    statusHttp: 200, memberListOverride: undefined as unknown, contentListOverride: undefined as unknown,
    intercept: null as null | ((request: Request) => Promise<Response> | null) }
  const apply = (command: Command) => {
    const input = command.input, action = input.action
    if (command.operation === 'moderateConcertUser') {
      const row = members.get(command.target)!
      members.set(command.target, { ...row, state: action === 'SUSPEND' ? 'SUSPENDED' : 'ACTIVE', revision: Number(input.expected_revision) + 1 })
    } else {
      const index = content.findIndex(row => row.target_id === command.target && row.target_type === input.target_type)
      content[index] = { ...content[index]!, state: action === 'HIDE' ? 'HIDDEN' : 'VISIBLE', revision: Number(input.expected_revision) + 1 }
    }
  }
  const receipt = (command: Command) => ({ receipt_id: id(901 + [...commands.keys()].indexOf(command.key)),
    actor_id: wire.actor, event_id: wire.event, operation_id: command.operation, idempotency_key: command.key, state: command.state,
    resource_id: command.state === 'SUCCEEDED' ? command.target : null,
    resource_revision: command.state === 'SUCCEEDED' ? Number(command.input.expected_revision) + 1 : null,
    result_status: command.state === 'SUCCEEDED' ? 200 : null, failure_reason: command.state === 'FAILED' ? 'DEPENDENCY_UNAVAILABLE' : null,
    created_at: new Date().toISOString(), expires_at: new Date(Date.now() + 86_400_000).toISOString() })
  base.reports.intercept = request => {
    const custom = state.intercept?.(request); if (custom) return custom
    const url = new URL(request.url), prefix = `/api/concert/events/${wire.event}/admin`
    if (request.method === 'GET' && url.pathname === `${prefix}/users`) {
      const user = url.searchParams.get('user_id')
      return Promise.resolve(Response.json(state.memberListOverride ?? { items: user ? [...members.values()].filter(row => row.user_id === user) : [...members.values()], next_cursor: null }))
    }
    if (request.method === 'GET' && url.pathname === `${prefix}/content`) return Promise.resolve(Response.json(state.contentListOverride ?? { items: content, next_cursor: null }))
    const userMatch = url.pathname.match(/\/admin\/users\/([^/]+)\/moderation$/)
    if (request.method === 'POST' && (userMatch || url.pathname === `${prefix}/content/moderation`)) return (async () => {
      const input = await request.clone().json() as Record<string, unknown>, key = request.headers.get('Idempotency-Key')!
      const operation = userMatch ? 'moderateConcertUser' : 'moderateConcertContent', target = userMatch?.[1] ?? String(input.target_id)
      const row = userMatch ? members.get(target) : content.find(row => row.target_id === target && row.target_type === input.target_type)
      if (!row) return base.error(404, 'RESOURCE_UNAVAILABLE')
      if (state.mode === 'conflict' || input.expected_revision !== row.revision) return base.error(409, 'VERSION_CONFLICT')
      if (state.mode === 'denied') return base.error(403, 'PERMISSION_DENIED')
      if (state.mode === 'unavailable') return base.error(503, 'DEPENDENCY_UNAVAILABLE')
      const command: Command = { key, operation, input, target, state: 'PENDING' }; commands.set(key, command)
      if (state.mode === 'lost') throw new TypeError('TEST_ONLY admitted moderation with lost response')
      if (state.mode === 'pending') return Response.json(receipt(command), { status: 202 })
      command.state = 'SUCCEEDED'; apply(command)
      return Response.json(userMatch ? members.get(target) : content.find(row => row.target_id === target && row.target_type === input.target_type))
    })()
    if (request.method === 'GET' && url.pathname.endsWith('/commands') && ['moderateConcertUser', 'moderateConcertContent'].includes(url.searchParams.get('operation_id') ?? '')) {
      const command = commands.get(url.searchParams.get('idempotency_key') ?? '')
      return Promise.resolve(state.statusHttp !== 200 ? base.error(state.statusHttp, 'RESOURCE_UNAVAILABLE')
        : command && command.operation === url.searchParams.get('operation_id') ? Response.json(receipt(command)) : base.error(404, 'RESOURCE_UNAVAILABLE'))
    }
    return null
  }
  return { ...base, moderation: state, members, content, commands,
    settle(key: string, outcome: 'SUCCEEDED' | 'FAILED') { const command = commands.get(key)!; command.state = outcome; if (outcome === 'SUCCEEDED') apply(command) },
    writes: () => base.requests.filter(r => r.method === 'POST' && /\/admin\/(users\/[^/]+|content)\/moderation$/.test(new URL(r.url).pathname)),
  }
}
