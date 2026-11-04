import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { PlacesOperations } from '../../../places/PlacesOperations'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { createOperationsController } from '../../model/controller'
import type { PlaceWritePort } from '../../model/ports'
import { deferred, fixture, time } from '../fixtures/data'

const owners: ReturnType<typeof fixture>[] = []
const controllers: ReturnType<typeof createOperationsController>[] = []
beforeEach(() => { vi.stubGlobal('location', { origin: 'https://mcr.example.invalid' }) })
afterEach(() => {
  controllers.splice(0).forEach(controller => controller.dispose())
  owners.splice(0).forEach(owner => owner.dispose())
  vi.unstubAllGlobals()
})
function setup() {
  const owner = fixture(); owners.push(owner)
  let unavailable = false
  const listeners = new Set<() => void>()
  const write = vi.fn<PlaceWritePort['write']>()
  const places: PlaceWritePort = { read() {
    if (unavailable) throw new Error('PRIVATE place producer diagnostic')
    return owner.release()
  }, subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener) } }, write }
  const controller = createOperationsController({ auth: owner.auth,
    authority: { read: owner.lease, subscribe: () => () => {} },
    clock: { read: () => ({ scope: owner.scope, nowMs: Date.parse(time()) }), subscribe: () => () => {} }, places })
  controllers.push(controller)
  return { owner, controller, write, fail(notify = false) { unavailable = true; if (notify) for (const listener of listeners) listener() },
    body() { const p = owner.current().place; return { expected_revision: p.revision, place: { ...p.place, closure_state: 'CLOSED' }, reason: 'SYNTHETIC reviewed closure' } } }
}

it('renders an unavailable map revision and rejects place edits when the producer throws', async () => {
  const f = setup(); await f.controller.loadPlaces(); f.fail()
  expect(() => f.controller.mapRevision()).not.toThrow()
  expect(f.controller.mapRevision()).toBeNull()
  const rendered = renderToStaticMarkup(createElement(PlacesOperations, { controller: f.controller }))
  expect(rendered).toContain('Unavailable')
  expect(rendered).not.toContain('PRIVATE')
  expect(() => f.controller.preparePlace(f.body())).not.toThrow()
  expect(f.controller.preparePlace(f.body())).toBe(false)
  expect(f.controller.read().command).toBeNull()
  expect(f.write).not.toHaveBeenCalled()
  expect(JSON.stringify(f.controller.read())).not.toContain('PRIVATE')
})

it('cancels a reviewed closure without writing when the producer becomes unreadable before confirm', async () => {
  const f = setup(); await f.controller.loadPlaces()
  expect(f.controller.preparePlace(f.body())).toBe(true)
  f.fail()
  await expect(f.controller.confirm()).resolves.toBeUndefined()
  expect(f.controller.read().command).toBeNull()
  expect(f.write).not.toHaveBeenCalled()
})

it('withdraws an unreadable place review synchronously while preserving other authorized operations', async () => {
  const f = setup(); await f.controller.loadPlaces()
  expect(f.controller.preparePlace(f.body())).toBe(true)
  expect(() => f.fail(true)).not.toThrow()
  expect(f.controller.read().command).toBeNull()
  expect(f.controller.read().access).toBe('ready')
  expect(f.write).not.toHaveBeenCalled()
  await f.controller.loadEvent()
  expect(f.controller.read().settings).not.toBeNull()
})

it('aborts an in-flight closure on unreadable producer withdrawal and never applies a late success', async () => {
  const f = setup(); await f.controller.loadPlaces()
  const held = deferred<Awaited<ReturnType<PlaceWritePort['write']>>>()
  let signal: AbortSignal | undefined
  f.write.mockImplementation(input => { signal = input.signal; return held.promise })
  const body = f.body()
  expect(f.controller.preparePlace(body)).toBe(true)
  const pending = f.controller.confirm()
  await vi.waitFor(() => expect(f.write).toHaveBeenCalledTimes(1))
  expect(() => f.fail(true)).not.toThrow()
  expect(signal?.aborted).toBe(true)
  expect(f.controller.read().command?.stage).toBe('unknown')
  held.resolve({ scope: f.owner.scope, status: 200, data: { place: body.place, revision: body.expected_revision + 1 } })
  await pending
  expect(f.controller.read().command?.stage).toBe('unknown')
  expect(f.controller.read().places[0]?.place.closure_state).toBe('OPEN')
  expect(f.controller.preparePlace(body)).toBe(false)
})

// The displayed map authority must update even before an operator opens a review.
it('notifies the view when an idle place producer withdraws without withdrawing event authority', async () => {
  const f = setup(); await f.controller.loadPlaces()
  const previous = f.controller.read(), listener = vi.fn()
  const stop = f.controller.subscribe(listener)
  f.fail(true)
  expect(listener).toHaveBeenCalledTimes(1)
  expect(f.controller.read()).not.toBe(previous)
  expect(f.controller.mapRevision()).toBeNull()
  expect(f.controller.read().access).toBe('ready')
  expect(f.write).not.toHaveBeenCalled()
  stop()
})
