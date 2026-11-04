// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { createModeration } from '../../model/controller'
import { harness } from '../fixtures/harness'

const owners: ReturnType<typeof harness>[] = []
const controllers: ReturnType<typeof createModeration>[] = []
afterEach(() => {
  controllers.splice(0).forEach(controller => controller.dispose())
  owners.splice(0).forEach(owner => owner.dispose())
  vi.restoreAllMocks()
})

it.each(['reports', 'members', 'content'] as const)('keeps %s unavailable when authority cannot publish withdrawal', async lane => {
  const owner = harness(lane); owners.push(owner)
  const released = vi.fn()
  const privacy = { ...owner.auth.privacy, registerCleanup: (cleanup: Parameters<typeof owner.auth.privacy.registerCleanup>[0]) => {
    const stop = owner.auth.privacy.registerCleanup(cleanup)
    return () => { stop(); released() }
  } }
  let controller!: ReturnType<typeof createModeration>
  expect(() => { controller = createModeration({ auth: { ...owner.auth, privacy }, lane,
    access: { read: owner.grant, subscribe() { throw new Error('PRIVATE unavailable producer diagnostic') } } }) }).not.toThrow()
  controllers.push(controller)
  await controller.load()
  expect(controller.read()).toMatchObject({ access: false, phase: 'denied', rows: [], detail: null, confirm: false })
  expect(owner.requests).toHaveLength(0)
  expect(released).toHaveBeenCalledTimes(1)
  expect(JSON.stringify(controller.read())).not.toContain('PRIVATE')
})

it('mounts denied after canonical privacy disposal without subscribing to authority', async () => {
  const owner = harness(); owners.push(owner)
  owner.auth.privacy.dispose()
  const subscribe = vi.fn(() => () => {})
  let controller!: ReturnType<typeof createModeration>
  expect(() => { controller = createModeration({ auth: owner.auth, lane: 'reports', access: { read: owner.grant, subscribe } }) }).not.toThrow()
  controllers.push(controller)
  await controller.load()
  expect(controller.read().access).toBe(false)
  expect(subscribe).not.toHaveBeenCalled()
  expect(owner.requests).toHaveLength(0)
})

it('releases the authority observer even when another cleanup throws', () => {
  const owner = harness(); owners.push(owner)
  const stopAuthority = vi.fn()
  const privacy = { ...owner.auth.privacy, registerCleanup: () => () => { throw new Error('PRIVATE cleanup diagnostic') } }
  const controller = createModeration({ auth: { ...owner.auth, privacy }, lane: 'reports',
    access: { read: owner.grant, subscribe: () => stopAuthority } })
  controllers.push(controller)
  expect(() => controller.dispose()).not.toThrow()
  expect(stopAuthority).toHaveBeenCalledTimes(1)
  expect(controller.read()).toMatchObject({ access: false, rows: [], detail: null })
})
