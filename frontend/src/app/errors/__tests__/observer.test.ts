import { afterEach, expect, it } from 'vitest'
import { harness } from './harness'
const owned: ReturnType<typeof harness>[] = []
const setup = () => { const h = harness(); owned.push(h); return h }
afterEach(() => owned.splice(0).forEach(h => h.dispose()))
it('distinguishes an offline browser hint from an unreachable server', () => {
  const h = setup(); expect(h.controller.read().kind).toBe('ready')
  h.patch({ phase: 'closed', baseline: null, problem: 'connection' })
  expect(h.controller.read()).toMatchObject({ kind: 'unavailable', stale: true, canRetry: true })
  h.connectivity.set('offline'); expect(h.controller.read()).toMatchObject({ kind: 'offline', canRetry: false })
  h.connectivity.set('online'); expect(h.controller.read().kind).toBe('unavailable'); expect(h.reconnect).not.toHaveBeenCalled()
})
it('bounds all pending phases with a single deadline that phase churn cannot restart', () => {
  const h = setup(); h.patch({ phase: 'ticket', baseline: null })
  expect(h.clock.pending).toBe(1); h.clock.advance(500); h.patch({ phase: 'opening' }); h.patch({ phase: 'authenticating' })
  expect(h.clock.pending).toBe(1); h.clock.advance(500)
  expect(h.controller.read()).toMatchObject({ kind: 'unavailable', pending: false }); expect(h.clock.pending).toBe(0)
  h.patch({ phase: 'ticket' }); h.clock.advance(90000); expect(h.clock.pending).toBe(0)
})
it('missing production release/view becomes actionable reload state without inventing retry', () => {
  const h = setup(); h.view.set(null); h.clock.advance(1000)
  expect(h.controller.read()).toMatchObject({ kind: 'unavailable', pending: false, canRetry: false })
  h.world.set(null); expect(h.controller.read().stale).toBe(false)
})
it('explicit retry calls the existing owner once and contains raw failures', async () => {
  const h = setup(); h.patch({ phase: 'closed', baseline: null }); let reject!: (error: Error) => void
  h.reconnect.mockImplementationOnce(() => new Promise<boolean>((_done, no) => { reject = no }))
  const pending = h.controller.retry(); await h.controller.retry(); expect(h.reconnect).toHaveBeenCalledTimes(1)
  expect(h.controller.read()).toMatchObject({ kind: 'reconnecting', pending: true })
  reject(new Error('https://secret.test/?token=hidden')); await pending
  expect(h.controller.read()).toMatchObject({ kind: 'unavailable', pending: false, canRetry: true })
  expect(JSON.stringify(h.controller.read())).not.toMatch(/secret|token|hidden/)
})
it('never treats reconnect completion as authoritative freshness', async () => {
  const h = setup(), old = h.view.read()!.baseline!
  h.patch({ phase: 'closed', baseline: null }); h.reconnect.mockResolvedValue(true)
  await h.controller.retry(); h.patch({ phase: 'live', baseline: { ...old, streamSeq: old.streamSeq + 1 } })
  expect(h.controller.read()).toMatchObject({ kind: 'resync', stale: true })
  h.patch({ baseline: { ...old, streamId: 'new-authoritative-stream' } })
  expect(h.controller.read()).toMatchObject({ kind: 'ready', stale: false })
})
it('keeps stale through live-before-snapshot and expires a stalled resync', () => {
  const h = setup(), old = h.view.read()!.baseline!
  h.patch({ phase: 'resync' }); h.patch({ phase: 'live' })
  expect(h.controller.read()).toMatchObject({ kind: 'resync', stale: true }); h.clock.advance(1000)
  expect(h.controller.read()).toMatchObject({ kind: 'unavailable', stale: true, canRetry: false })
  h.patch({ baseline: { ...old, streamId: 'fresh', cause: 'RESYNC' } }); expect(h.controller.read().kind).toBe('ready')
})
it('leaves a valid original room live during room switching and a pre-send rejection', () => {
  const h = setup(); h.patch({ phase: 'switching' }); expect(h.controller.read().kind).toBe('ready')
  h.patch({ phase: 'live', problem: 'room-unavailable' }); expect(h.controller.read()).toMatchObject({ kind: 'ready', stale: false })
  expect(h.clock.pending).toBe(0)
})
it('uses explicit scoped maintenance/version decisions only and never auto recovers', async () => {
  const h = setup(); h.service.set({ release: h.binding.release, kind: 'maintenance' })
  expect(h.controller.read()).toMatchObject({ kind: 'maintenance', canRetry: false }); await h.controller.retry()
  h.service.set({ release: { ...h.binding.release }, kind: 'version' }); expect(h.controller.read().kind).toBe('resync')
  h.patch({ baseline: { ...h.view.read()!.baseline!, streamId: 'after-maintenance' } }); expect(h.controller.read().kind).toBe('ready')
  h.service.set({ release: h.binding.release, kind: 'version' }); expect(h.controller.read().kind).toBe('version')
  expect(h.reconnect).not.toHaveBeenCalled()
})
it('map mismatch is reload-only even when offline and a recovery callback exists', async () => {
  const h = setup(); h.patch({ phase: 'closed', problem: 'map-version', baseline: null }); h.connectivity.set('offline')
  expect(h.controller.read()).toMatchObject({ kind: 'version', canRetry: false }); await h.controller.retry(); expect(h.reconnect).not.toHaveBeenCalled()
})
it('suspension installs no display timer and leaves foreground recovery with FE17', () => {
  const h = setup(); h.patch({ phase: 'suspended', visible: false, baseline: null })
  expect(h.controller.read().kind).toBe('suspended'); expect(h.clock.pending).toBe(0)
  h.patch({ phase: 'ticket', visible: true }); expect(h.controller.read().kind).toBe('reconnecting'); expect(h.reconnect).not.toHaveBeenCalled()
})
it('fences late retry completion and old notifications after release replacement', async () => {
  const h = setup(); h.patch({ phase: 'closed', baseline: null }); let done!: (ok: boolean) => void
  h.reconnect.mockImplementationOnce(() => new Promise(resolve => { done = resolve }))
  const pending = h.controller.retry(), callbacks = [...h.view.listeners]
  const next = { ...h.binding, release: { ...h.binding.release, signal: new AbortController().signal } }
  h.world.set(next); expect(h.view.listeners.size).toBe(1); expect(h.clock.pending).toBe(0)
  callbacks.forEach(fn => fn()); done(true); expect(await pending).toBe(false)
  expect(h.controller.read()).toMatchObject({ kind: 'unavailable', stale: false, canRetry: false })
})
for (const loss of ['abort', 'policy', 'identity', 'event'] as const) it(`withdraws protected state synchronously on ${loss} loss`, async () => {
  const h = setup(); h.patch({ phase: 'resync' }); const callbacks = [...h.view.listeners]
  if (loss === 'abort') h.lifetime.abort()
  else if (loss === 'policy') h.auth.privacy.changePolicy(h.auth.privacy.capture(), 'consent-revoked')
  else h.auth.session.replace(h.auth.session.read().generation, { userId: loss === 'identity' ? '00000000-0000-4000-8000-000000000090' : h.binding.release.scope.userId!, eventId: loss === 'event' ? '00000000-0000-4000-8000-000000000091' : h.binding.release.scope.eventId!, accessToken: 'next-test-only' })
  callbacks.forEach(fn => fn()); expect(h.controller.read()).toEqual({ kind: 'unavailable', pending: false, stale: false, canRetry: false })
  expect(h.view.listeners.size).toBe(0); expect(h.clock.pending).toBe(0); expect(await h.controller.retry()).toBe(false)
})
it('rejects mismatched scope/baseline authority and releases each subscription once', () => {
  const h = setup(); h.controller.start(); expect(h.view.listeners.size).toBe(1); expect(h.world.listeners.size).toBe(1)
  h.patch({ scope: { ...h.view.read()!.scope, userId: 'foreign' } }); expect(h.controller.read().kind).toBe('reconnecting')
  h.controller.dispose(); h.controller.dispose(); expect(h.view.listeners.size).toBe(0); expect(h.world.listeners.size).toBe(0)
  expect(h.connectivity.listeners.size).toBe(0); expect(h.service.listeners.size).toBe(0); expect(h.recovery.listeners.size).toBe(0)
})
it('old recovery action never crosses a new release', async () => {
  const h = setup(); h.patch({ phase: 'closed', baseline: null }); h.world.set({ ...h.binding, release: { ...h.binding.release } })
  expect(await h.controller.retry()).toBe(false); expect(h.reconnect).not.toHaveBeenCalled()
})
for (const problem of ['authority', 'sink'] as const) it(`does not offer reconnect after ${problem} loss`, async () => {
  const h = setup(); h.patch({ phase: 'closed', baseline: null, problem })
  expect(h.controller.read().canRetry).toBe(false); expect(await h.controller.retry()).toBe(false); expect(h.reconnect).not.toHaveBeenCalled()
})
it('timeout bounds a hung explicit retry without scheduling automatic attempts', async () => {
  const h = setup(); h.patch({ phase: 'closed', baseline: null }); h.reconnect.mockImplementationOnce(() => new Promise(() => {}))
  void h.controller.retry(); h.clock.advance(1000); expect(h.controller.read()).toMatchObject({ kind: 'unavailable', pending: false })
  expect(h.reconnect).toHaveBeenCalledTimes(1); h.clock.advance(90000); expect(h.reconnect).toHaveBeenCalledTimes(1)
})
