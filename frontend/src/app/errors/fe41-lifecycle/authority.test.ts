// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { harness, port } from '../__tests__/harness'
import { createDefaultConnectionStatus } from '../default-entry'
import type { WorldLifecycleStatus } from './status'
const stops: (() => void)[] = []
afterEach(() => stops.splice(0).reverse().forEach(stop => stop()))
it('borrows only the exact captured lifecycle release, reacts without replication churn, and retires subscriptions', () => {
  const h = harness(); stops.push(h.dispose); h.controller.dispose()
  const lifecycle = port<WorldLifecycleStatus | null>({ release: h.binding.release, kind: 'ready' })
  h.world.set({ ...h.binding, lifecycle })
  const entry = createDefaultConnectionStatus({ auth: h.auth, world: h.world, browser: window, reload() {} })
  stops.push(entry.dispose); entry.start()
  expect(entry.controller.read().kind).toBe('ready'); expect(lifecycle.listeners.size).toBe(1)
  lifecycle.set({ release: { ...h.binding.release }, kind: 'version' })
  expect(entry.controller.read().kind).toBe('ready')
  lifecycle.set({ release: h.binding.release, kind: 'version' })
  expect(entry.controller.read()).toMatchObject({ kind: 'version', stale: true, canRetry: false })
  const callbacks = [...lifecycle.listeners]
  h.world.set(null); expect(lifecycle.listeners.size).toBe(0)
  callbacks.forEach(fn => fn())
  expect(entry.controller.read()).toEqual({ kind: 'unavailable', stale: false, pending: false, canRetry: false })
  entry.dispose(); expect(h.view.listeners.size).toBe(0); expect(h.world.listeners.size).toBe(0)
})
