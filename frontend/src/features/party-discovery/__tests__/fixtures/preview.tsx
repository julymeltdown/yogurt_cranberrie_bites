import { createRoot } from 'react-dom/client'
import { createBrowserRouter, RouterProvider, useParams } from 'react-router-dom'
import { createAppRoutes } from '../../../../app/router'
import { bootstrapFor } from '../../../../app/__tests__/fixtures/ports'
import { projectCapabilities } from '../../../../app/capabilities'
import type { ShellPorts } from '../../../../app/ports'
import { createSessionController } from '../../../auth/model/session'
import { createWorldRuntime, type WorldRuntime } from '../../../../world/runtime'
import { attachMovement, type MovementAttachment } from '../../../../world/input/controller'
import { loadMap } from '../../../../world/map/resources'
import { movementData } from '../../../../world/__tests__/fixtures/movement-data'
import { rehash } from '../../../../world/__tests__/fixtures/synthetic'
import { WorldHelp } from '../../../world-help/WorldHelp'
import { createPartyController, type PartyAccess } from '../../model/controller'
import type { Application, CommandOperation } from '../../model/contract'
import { PartyDetail, PartyDiscovery } from '../../ui/PartyDiscovery'
import { activity, application, errorResponse, ids, party, pendingReceipt } from './data'
import '../../../../shared/styles/base.css'
import '../../../../app/shell.css'

const terrain = await movementData(), pin = await rehash(terrain.map, terrain.theme)
let applicationSequence = 32
let mode = 'ok', statusMode = 'PENDING', current: Application | null = null, access: PartyAccess | null = null
let runtime: WorldRuntime | null = null, movement: MovementAttachment | null = null, controls: HTMLDivElement | null = null
let mounts = 0, disposals = 0, connections = 0, peakConnections = 0, disposed = false
const accessListeners = new Set<() => void>(), requests: { method: string; path: string; query: string; key: string | null; body: string }[] = []
const captured = new Map<string, { operation: CommandOperation; state: 'PENDING' | 'CANCELLED'; applicationId: string; revision: number }>()
const auth = createSessionController({ eventId: ids.event, cookiePolicy: { available: true, onInvalidate: () => () => {}, run: (_kind, work) => work(), dispose() {} }, fetch: async input => {
  const request = input as Request, url = new URL(request.url), path = url.pathname, key = request.headers.get('Idempotency-Key')
  requests.push({ method: request.method, path, query: url.search, key, body: await request.clone().text() })
  if (mode === 'private') return errorResponse(403, 'PERMISSION_DENIED')
  if (mode === 'rate') return errorResponse(429, 'RATE_LIMITED')
  if (mode === 'outage') return errorResponse(503, 'DEPENDENCY_UNAVAILABLE')
  if (path.endsWith('/commands')) {
    if (statusMode === '404') return errorResponse(404, 'RESOURCE_UNAVAILABLE')
    if (statusMode === 'outage') return errorResponse(503, 'DEPENDENCY_UNAVAILABLE')
    const stored = captured.get(url.searchParams.get('idempotency_key')!), operation = stored?.operation ?? 'applyConcertParty'
    if (statusMode === 'SUCCEEDED') current = application({ application_id: stored!.applicationId, state: stored?.state ?? 'PENDING', revision: stored!.revision })
    return Response.json(pendingReceipt(url.searchParams.get('idempotency_key')!, { operation_id: operation,
      ...(statusMode === 'SUCCEEDED' ? { state: 'SUCCEEDED', resource_id: current!.application_id, resource_revision: current!.revision, result_status: 200 }
        : statusMode === 'RETIRED' ? { state: 'RETIRED' } : {}) }))
  }
  if (request.method !== 'GET') {
    const operation = request.method === 'POST' ? 'applyConcertParty' : 'cancelConcertApplication', state = request.method === 'POST' ? 'PENDING' : 'CANCELLED'
    const applicationId = request.method === 'POST' ? `00000000-0000-4000-8000-${(++applicationSequence).toString(16).padStart(12, '0')}` : path.split('/').at(-1)!
    const revision = request.method === 'POST' ? 1 : (current?.revision ?? 1) + 1
    captured.set(key!, { operation, state, applicationId, revision })
    if (mode === 'lost') throw new TypeError('fixture lost response')
    if (mode === 'pending') return Response.json(pendingReceipt(key!, { operation_id: operation }), { status: 202 })
    current = application({ application_id: applicationId, state, revision }); return Response.json(current)
  }
  if (path.endsWith('/me/parties')) {
    const a = activity()
    return Response.json({ ...a,
      owned: { ...a.owned, next_cursor: mode === 'partial' && !url.searchParams.has('owned_cursor') ? 'owned_next' : null },
      memberships: { ...a.memberships, next_cursor: mode === 'partial' && !url.searchParams.has('membership_cursor') ? 'members_next' : null },
      applications: { items: current ? [current] : [], next_cursor: mode === 'partial' && !url.searchParams.has('application_cursor') ? 'apps_next' : null },
    })
  }
  const p = party(mode === 'full' ? { member_count: 5 } : mode === 'cancelled' ? { state: 'CANCELLED' } : {})
  if (path.endsWith('/parties')) {
    const state = url.searchParams.get('state'), section = url.searchParams.get('ticket_section')
    const items = mode === 'empty' || state && p.state !== state || section && p.ticket_section !== section || url.searchParams.has('has_space') && p.member_count >= p.capacity ? [] : [p]
    return Response.json({ items, next_cursor: null })
  }
  return Response.json(p)
} })
auth.session.replace(0, { userId: ids.actor, eventId: ids.event, accessToken: 'fixture-token-memory-only' })
const controller = createPartyController({ auth, access: { read: () => access, subscribe(fn) { accessListeners.add(fn); return () => { accessListeners.delete(fn) } } } })
function release(readOnly = false) {
  const s = auth.session.read(), b = bootstrapFor({ ...s, userId: s.userId!, eventId: s.eventId! })
  access = { scope: auth.privacy.capture(), phase: readOnly ? 'READ_ONLY' : 'OPEN', capabilities: projectCapabilities({ ...b.capabilities, ...(readOnly ? { mutations: { available: false, reason: 'EVENT_READ_ONLY' } } : {}) }) }
  accessListeners.forEach(fn => fn()); return { ...b, event: { ...b.event, title: 'FE21 synthetic concert', phase: access.phase }, capabilities: access.capabilities }
}
const ports: ShellPorts = { auth, session: auth.session, readiness: { read: () => 'ready', subscribe: () => () => {} },
  bootstrap: { load: async () => release() }, admin: { authorize: async () => false },
  world: { mount(host, context) {
    mounts++; connections++; peakConnections = Math.max(peakConnections, connections)
    controls = document.createElement('div'); controls.style.cssText = 'position:absolute;bottom:0;left:0;right:0'; host.append(controls)
    const owner = createWorldRuntime({ host,
      load: signal => loadMap({ mapText: JSON.stringify(terrain.map), themeText: JSON.stringify(terrain.theme), pin,
        resources: { read: async resource => resource.kind === 'png' ? terrain.png : terrain.tiledBytes } }, signal),
      attachControls(context) { const attached = attachMovement(context, { controls: controls!, zoom: 1,
        policy: { speedPxPerSecond: 120, footWidthPx: 8, footHeightPx: 6, interactionRadiusPx: 42 }, consume() {} }); movement = attached
        return { setEnabled: value => attached.setEnabled(value), dispose() { attached.dispose(); if (movement === attached) movement = null } } },
    }); runtime = owner; void owner.start(context.signal)
    let closed = false
    return { setInputBlocked: value => owner.setInputBlocked(value), setVisible: value => owner.setVisible(value), dispose() {
      if (closed) return; closed = true; disposals++; connections--; owner.dispose(); controls?.remove(); controls = null
    } }
  } },
}
function DetailPanel() { return <PartyDetail controller={controller} partyId={useParams().partyId ?? ''} /> }
history.replaceState(null, '', '/meet')
const router = createBrowserRouter(createAppRoutes(ports, { panels: { '/meet': <PartyDiscovery controller={controller} />, '/meet/parties/:partyId': <DetailPanel />,
  '/world/help': <WorldHelp /> } }))
const root = createRoot(document.getElementById('root')!); root.render(<RouterProvider router={router} />)
const style = document.createElement('style'); style.textContent = '#qualification{font:12px/1.4 system-ui;padding:8px;background:#faf6ed;position:relative;z-index:3}.world-shell{height:calc(100dvh - 50px)}'; document.head.append(style)
const bridge = {
  state: () => ({ party: controller.read(), world: runtime?.read(), foot: movement?.read().foot, mounts, disposals, connections, peakConnections, canvases: document.querySelectorAll('canvas').length, requests }),
  async mode(value: string) { mode = value; await Promise.all([controller.loadList(), controller.loadActivity(), controller.loadDetail(ids.party)]) },
  status(value: string) { statusMode = value }, open() { release() }, readOnly() { release(true) },
  revoke() { auth.privacy.changePolicy(auth.privacy.capture(), 'block') },
  async dispose() { if (disposed) return; disposed = true; controller.dispose(); root.unmount(); router.dispose(); runtime?.dispose(); await runtime?.settled(); auth.dispose(); style.remove() },
}
window.__FE21__ = bridge
declare global { interface Window { __FE21__: typeof bridge } }
// Fixture connection counters qualify shell ownership only; no real WebSocket or backend is claimed.
