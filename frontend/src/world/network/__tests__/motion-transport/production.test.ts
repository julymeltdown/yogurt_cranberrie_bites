// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { createSessionController } from '../../../../features/auth/model/session'
import { decodeBootstrap } from '../../../../features/auth/model/bootstrap'
import { authResponse, capabilities, eventId, fixture, isolatedCookiePolicy } from '../../../../features/auth/__tests__/fixtures/data'
import { withAdmission } from '../../../../features/auth/__tests__/fixtures/admission'
import { createProductionWorld } from '../../../../app/world-entry/production'

const composition = vi.hoisted(() => {
  const subscribe = () => () => {}
  const mount = vi.fn(() => ({ dispose() {}, setVisible() {}, setInputBlocked() {} }))
  return { mount, create: vi.fn((_options: { motionProfileVersion?: string }) => ({ mount, subscribe: () => () => {},
    replication: { read: () => null, subscribe, switchRoom: async () => false, reconnect: async () => false },
    policy: { read: () => null, subscribe }, presentation: { read: () => null, subscribe }, dispose() {},
  })) }
})
vi.mock('../../../composition', () => ({ createWorldComposition: composition.create }))
vi.mock('../../../../app/world-entry/resolver', () => ({ createWorldEntryResolver: () => async () => { throw Error('Composition boundary only') } }))
vi.mock('../../../avatar/whole-character', () => ({ createWholeCharacterWorldRenderer: vi.fn() }))
vi.mock('../../../avatar/motion-adoption/renderer', () => ({ createMotionWorldRenderer: vi.fn() }))
vi.mock('../../../../app/world-entry/reference-scene-overview', () => ({ createRootSceneOverview: vi.fn() }))
vi.mock('../../../../app/world-entry/mural-production/factory', () => ({ createProductionMuralFactory: () => null }))
const dispose: (() => void)[] = []
afterEach(() => { dispose.splice(0).reverse().forEach(fn => fn()); vi.clearAllMocks(); document.body.replaceChildren() })

it('the actual default production loader explicitly requests motion; the composition is the observed downstream boundary', async () => {
  const auth = createSessionController({ eventId, qualifiedRefresh: true, cookiePolicy: isolatedCookiePolicy(), fetch: withAdmission(async input => {
    const path = new URL((input as Request).url).pathname
    if (path === '/api/auth/capabilities') return Response.json(capabilities())
    if (path === '/api/auth/login') return Response.json(authResponse())
    return new Response(null, { status: 204 })
  }) })
  dispose.push(() => auth.dispose())
  auth.start(); await vi.waitFor(() => expect(auth.read().emailEnabled).toBe(true))
  expect(await auth.login('fixture@example.test', 'fixture-password')).toBe(true)
  const state = auth.session.read()
  if (!state.userId || !state.eventId) throw Error('Expected canonical fixture identity')
  const scope = { generation: state.generation, userId: state.userId, eventId: state.eventId }
  const world = createProductionWorld({ auth, localInput: { revision: 'fe17-optin-test', reviewedBy: 'test-only', zoom: 1,
    movementPolicy: { speedPxPerSecond: 96, footWidthPx: 12, footHeightPx: 8, interactionRadiusPx: 96 } } })
  dispose.push(() => world.dispose())
  const host = document.createElement('div'); document.body.append(host)
  const bootstrap = await decodeBootstrap(fixture(), scope)
  world.world.mount(host, { scope, privacyScope: auth.privacy.capture(), bootstrap, signal: new AbortController().signal })
  await vi.waitFor(() => expect(composition.mount).toHaveBeenCalledTimes(1), { timeout: 5000 })
  expect(composition.create).toHaveBeenCalledTimes(1)
  expect(composition.create.mock.calls[0]![0].motionProfileVersion).toBe('1.0.0')
})
