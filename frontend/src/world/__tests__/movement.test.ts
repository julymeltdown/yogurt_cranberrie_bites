// @vitest-environment jsdom
import { webcrypto } from 'node:crypto'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { movementData } from './fixtures/movement-data'
import { loadMap } from '../map/resources'
import { createCollision, normalized, type SimulationPolicy } from '../input/simulation'
import { createMovementModel, STEP_MS, type MovementIntent } from '../input/model'
import { nearbyTargets } from '../interaction/selection'
import { attachMovement } from '../input/controller'
import { createCameraControl, type WorldCamera } from '../camera/port'
import { validate } from '../network/transport/contract'
import inputFixture from '../../../../contracts/fixtures/realtime/movement-heartbeat.json'
import { createWorldRuntime, type AttachControls, type EngineFactory, type WorldRuntime } from '../runtime'
const policy: SimulationPolicy = { speedPxPerSecond: 120, footWidthPx: 8, footHeightPx: 6, interactionRadiusPx: 42 }
const cleanup: (() => void)[] = []
let frames: Map<number, FrameRequestCallback>, clock = 0, frameId = 0
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(r => { resolve = r }); return { promise, resolve } }
beforeEach(() => {
  frames = new Map(); clock = 0
  vi.stubGlobal('crypto', webcrypto)
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => { frames.set(++frameId, callback); return frameId })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id))
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
})
afterEach(() => { for (const dispose of cleanup.splice(0)) dispose(); document.body.replaceChildren(); vi.unstubAllGlobals(); vi.restoreAllMocks() })
async function mapData() {
  const fixture = await movementData(), dispose = vi.fn()
  const map = await loadMap({ mapText: JSON.stringify(fixture.map), themeText: JSON.stringify(fixture.theme), pin: fixture.pin,
    resources: { read: async resource => resource.kind === 'png' ? fixture.png : fixture.tiledBytes },
    decode: async () => ({ image: { naturalWidth: 96, naturalHeight: 32 } as HTMLImageElement, dispose }) }, new AbortController().signal)
  return { map, dispose }
}
function advance(count = 1, delta = STEP_MS) {
  for (let i = 0; i < count; i++) { const callbacks = Array.from(frames.values()); frames.clear(); for (const callback of callbacks) callback(clock); clock += delta }
}
function eventPointer(target: EventTarget, type: string, id: number, x = 100, y = 100, patch: MouseEventInit = {}) {
  const event = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 0, ...patch })
  Object.defineProperty(event, 'pointerId', { value: id }); target.dispatchEvent(event); return event
}
function key(target: EventTarget, type: string, code: string, patch: KeyboardEventInit = {}) { const event = new KeyboardEvent(type, { bubbles: true, cancelable: true, code, ...patch }); target.dispatchEvent(event); return event }
function cameraFor(surface: HTMLCanvasElement): WorldCamera {
  return { surface: () => surface, center: vi.fn(), project: p => p.x >= 0 && p.y >= 0 ? p : null, unproject: p => p,
    read: () => ({ zoom: 1, scrollX: 0, scrollY: 0, width: 400, height: 300 }) }
}
async function controls(patch: Partial<Parameters<typeof attachMovement>[1]> = {}) {
  const { map } = await mapData(), surface = document.createElement('canvas'), container = document.createElement('div'), abort = new AbortController()
  document.body.append(surface, container)
  const captures = new Map<HTMLElement, Set<number>>()
  for (const element of [surface, HTMLElement.prototype]) {
    Object.defineProperties(element, {
      setPointerCapture: { configurable: true, value(this: HTMLElement, id: number) { const set = captures.get(this) ?? new Set(); set.add(id); captures.set(this, set) } },
      hasPointerCapture: { configurable: true, value(this: HTMLElement, id: number) { return captures.get(this)?.has(id) ?? false } },
      releasePointerCapture: { configurable: true, value(this: HTMLElement, id: number) { captures.get(this)?.delete(id) } },
    })
  }
  const context = { host: surface, map, camera: cameraFor(surface), signal: abort.signal }, intents: MovementIntent[] = []
  const attachment = attachMovement(context, { controls: container, policy, zoom: 1, consume: intent => intents.push(intent), ...patch })
  cleanup.push(() => { attachment.dispose(); abort.abort() })
  attachment.setEnabled(true)
  const stick = container.querySelector<HTMLButtonElement>('.world-joystick')!
  vi.spyOn(stick, 'getBoundingClientRect').mockReturnValue({ left: 50, top: 50, width: 100, height: 100, right: 150, bottom: 150, x: 50, y: 50, toJSON() {} })
  return { attachment, surface, container, abort, intents, stick, captures, context }
}
it('uses current C05 20Hz tuning target without constructing a wire frame', () => { expect(STEP_MS).toBe(50); expect(normalized(1, 1).x).toBeCloseTo(Math.SQRT1_2, 12); expect(normalized(1, 1).y).toBeCloseTo(Math.SQRT1_2, 12) })
it.each([
  ['speedPxPerSecond', 0], ['speedPxPerSecond', -1], ['speedPxPerSecond', Infinity], ['speedPxPerSecond', NaN], ['speedPxPerSecond', 1025],
  ['footWidthPx', 0], ['footWidthPx', 33], ['footHeightPx', 0], ['footHeightPx', 33], ['interactionRadiusPx', 0], ['interactionRadiusPx', 513], ['interactionRadiusPx', NaN],
] as const)('rejects unsafe injected local policy %s=%s', async (name, value) => { const { map } = await mapData(); expect(() => createMovementModel(map, { ...policy, [name]: value })).toThrow() })
it('normalizes diagonals and bounds dt while preserving valid whole-foot positions', async () => {
  const { map } = await mapData(), collision = createCollision(map.map.geometry, policy), foot = { x: 80, y: 112 }
  const moved = collision.move(foot, { x: 1, y: 1 }, .05)
  expect(Math.hypot(moved.x - foot.x, moved.y - foot.y)).toBeCloseTo(6, 8)
  expect(collision.move(foot, { x: 1, y: 0 }, 5)).toEqual(collision.move(foot, { x: 1, y: 0 }, .1))
  for (const seconds of [NaN, Infinity, -1, 0]) expect(collision.move(foot, { x: 1, y: 0 }, seconds)).toEqual(foot)
  expect(collision.move(foot, { x: Infinity, y: 0 }, .05)).toEqual(foot)
})
it('cannot tunnel through a one-cell wall even at maximum local speed', async () => {
  const { map } = await mapData(), collision = createCollision(map.map.geometry, { ...policy, speedPxPerSecond: 1024 })
  const next = collision.move({ x: 240, y: 112 }, { x: 1, y: 0 }, .1)
  expect(next.x).toBeLessThanOrEqual(252); expect(next.x).toBeGreaterThan(240); expect(collision.walkable(next)).toBe(true)
})
it('bounds foot rectangle on all world edges even with an open border', async () => {
  const { map } = await mapData(), g = structuredClone(map.map.geometry); g.collision_grid = g.collision_grid.map(row => row.map(() => 0))
  const c = createCollision(g, policy); let foot = { x: 4, y: 6 }
  for (let i = 0; i < 100; i++) foot = c.move(foot, { x: -1, y: -1 }, .1)
  expect(foot.x).toBe(4); expect(foot.y).toBe(6)
  foot = { x: 764, y: 448 }; for (let i = 0; i < 100; i++) foot = c.move(foot, { x: 1, y: 1 }, .1)
  expect(foot).toEqual({ x: 764, y: 448 })
})
it('never crosses diagonally touching blocked corners', async () => {
  const { map } = await mapData(), g = structuredClone(map.map.geometry)
  g.width_cells = 3; g.height_cells = 3; g.collision_grid = [[0, 1, 1], [1, 0, 0], [1, 0, 0]]
  const c = createCollision(g, policy); let foot = { x: 16, y: 16 }
  for (let i = 0; i < 50; i++) foot = c.move(foot, { x: 1, y: 1 }, .1)
  expect(foot.x).toBeLessThan(32); expect(foot.y).toBeLessThanOrEqual(32); expect(c.route({ x: 16, y: 16 }, { x: 48, y: 48 })).toBeNull()
})
it.each([{ x: 272, y: 112 }, { x: -1, y: 100 }, { x: NaN, y: 100 }, { x: 9999, y: 100 }])('rejects blocked/outside tap %j without an intent', async point => {
  const model = createMovementModel((await mapData()).map, policy); expect(model.tap(point)).toBe(false); expect(model.active()).toBe(false)
})
it('routes around the wall through its opening without corner cutting or oversized axes', async () => {
  const model = createMovementModel((await mapData()).map, policy); expect(model.tap({ x: 336, y: 112 })).toBe(true)
  expect(model.read().path.some(p => p.y === 240)).toBe(true)
  for (let i = 0; i < 400 && model.active(); i++) { const intent = model.step(); expect(Math.hypot(intent.axisX, intent.axisY)).toBeLessThanOrEqual(1 + 1e-9); expect(model.collision.walkable(model.read().foot)).toBe(true) }
  expect(model.read().foot.x).toBeCloseTo(336); expect(model.read().foot.y).toBeCloseTo(112); expect(model.active()).toBe(false)
})
it('selects only open in-range unobstructed C06 targets and consumes interactions once', async () => {
  const { map } = await mapData(), model = createMovementModel(map, policy)
  expect(model.read().nearby).toHaveLength(0); expect(model.interact()).toBe(false)
  model.axis(1, 0); for (let i = 0; i < 5; i++) model.step(); model.axis(0, 0)
  expect(model.read().foot.x).toBeCloseTo(110); expect(model.read().selectedId).toBe('inspect_concept')
  model.interact(); model.interact(); expect(model.step().buttons).toEqual(['interact']); expect(model.step().buttons).toEqual([])
  model.interact(); model.reset(); expect(model.step().buttons).toEqual([])
  const closed = { ...map, map: { ...map.map, places: map.map.places.map(p => ({ ...p, closure_state: 'CLOSED' })) } }
  expect(nearbyTargets(closed, model.read().foot, model.collision, 512)).toHaveLength(0)
  const behind = { ...map, map: { ...map.map, geometry: { ...map.map.geometry, interactions: [{ ...map.map.geometry.interactions[0]!, cell_x: 9, cell_y: 3 }] } } }
  expect(nearbyTargets(behind, { x: 240, y: 112 }, model.collision, 100)).toHaveLength(0)
})
it('returns immutable local snapshots and never emits coordinates in C05 intent', async () => {
  const model = createMovementModel((await mapData()).map, policy), snapshot = model.read()
  expect(Object.isFrozen(snapshot.foot)).toBe(true); model.axis(1, 0)
  expect(Object.keys(model.step()).sort()).toEqual(['axisX', 'axisY', 'buttons']); expect(snapshot.foot).toEqual({ x: 80, y: 112 })
})
it('uses only focused map/joystick keys and neutralizes immediately on panel focus', async () => {
  const c = await controls(), panel = document.createElement('input'); document.body.append(panel)
  panel.focus(); key(panel, 'keydown', 'KeyD'); advance(4); expect(c.attachment.read().foot.x).toBe(80)
  c.surface.focus(); key(c.surface, 'keydown', 'KeyD'); advance(4); expect(c.attachment.read().foot.x).toBeGreaterThan(80)
  panel.focus(); expect(c.intents.at(-1)).toEqual({ axisX: 0, axisY: 0, buttons: [] }); const foot = c.attachment.read().foot
  key(panel, 'keydown', 'KeyD'); advance(10); expect(c.attachment.read().foot).toEqual(foot); expect(frames.size).toBe(0)
})
it.each([{ ctrlKey: true }, { metaKey: true }, { altKey: true }, { shiftKey: true }, { isComposing: true }, { repeat: true }])('ignores composing/modifier/repeated initial movement keys %j', async patch => {
  const c = await controls(); c.surface.focus(); key(c.surface, 'keydown', 'KeyD', patch); advance(4); expect(c.attachment.read().foot.x).toBe(80)
})
it.each(['textarea', 'select', 'button', 'dialog', 'contenteditable'])('ordinary %s keyboard actions do not move the player', async tag => {
  const c = await controls(), element = document.createElement(tag === 'contenteditable' ? 'div' : tag)
  element.tabIndex = 0; if (tag === 'contenteditable') element.contentEditable = 'true'; document.body.append(element); element.focus()
  key(element, 'keydown', 'ArrowRight'); advance(3); expect(c.attachment.read().foot.x).toBe(80)
})
it.each(['pointercancel', 'lostpointercapture', 'pointerup'])('owns one pointer and clears a diagonal stick on %s', async action => {
  const c = await controls(); eventPointer(c.stick, 'pointerdown', 1, 148, 148); advance(4)
  const first = c.attachment.read().foot; expect(first.x).toBeGreaterThan(80); expect(first.y).toBeGreaterThan(112)
  eventPointer(c.stick, 'pointerdown', 2, 50, 50); eventPointer(window, 'pointermove', 2, 50, 50)
  expect(c.captures.get(c.stick)?.has(1)).toBe(true); expect(c.captures.get(c.stick)?.has(2)).not.toBe(true)
  eventPointer(action === 'lostpointercapture' ? c.stick : window, action, 1, 148, 148)
  expect(c.intents.at(-1)).toEqual({ axisX: 0, axisY: 0, buttons: [] }); const foot = c.attachment.read().foot
  advance(8); expect(c.attachment.read().foot).toEqual(foot); expect(c.captures.get(c.stick)?.size ?? 0).toBe(0); expect(frames.size).toBe(0)
})
it.each(['block', 'blur', 'hidden', 'abort', 'dispose'])('clears movement and queued one-shot on %s without stale resume', async action => {
  const c = await controls(); c.surface.focus(); key(c.surface, 'keydown', 'KeyD'); advance(7); key(c.surface, 'keydown', 'KeyE')
  if (action === 'block') c.attachment.setEnabled(false)
  else if (action === 'blur') window.dispatchEvent(new Event('blur'))
  else if (action === 'hidden') { vi.spyOn(document, 'hidden', 'get').mockReturnValue(true); document.dispatchEvent(new Event('visibilitychange')) }
  else if (action === 'abort') c.abort.abort(); else c.attachment.dispose()
  const foot = c.attachment.read().foot, interactions = c.intents.filter(i => i.buttons.length).length; advance(10)
  expect(c.attachment.read().foot).toEqual(foot); expect(frames.size).toBe(0)
  if (action === 'hidden') { vi.spyOn(document, 'hidden', 'get').mockReturnValue(false); document.dispatchEvent(new Event('visibilitychange')) }
  if (action === 'blur') window.dispatchEvent(new Event('focus'))
  c.attachment.setEnabled(true); advance(8); expect(c.attachment.read().foot).toEqual(foot); expect(c.intents.filter(i => i.buttons.length)).toHaveLength(interactions)
})
it('rejects dragged/long/different-pointer taps and routes only a valid owned tap', async () => {
  const c = await controls()
  eventPointer(c.surface, 'pointerdown', 1, 120, 112); eventPointer(window, 'pointerup', 2, 120, 112); expect(c.attachment.read().path).toHaveLength(0)
  eventPointer(window, 'pointermove', 1, 140, 112); eventPointer(window, 'pointerup', 1, 140, 112); expect(c.attachment.read().path).toHaveLength(0)
  eventPointer(c.surface, 'pointerdown', 3, 120, 112); eventPointer(window, 'pointerup', 3, 120, 112); expect(c.attachment.read().path.length).toBeGreaterThan(0)
  advance(30); expect(c.attachment.read().foot.x).toBeCloseTo(120)
})
it('coalesces queued button activations and ignores repeated interact key', async () => {
  const c = await controls(); c.surface.focus(); key(c.surface, 'keydown', 'KeyD'); advance(7); key(c.surface, 'keyup', 'KeyD')
  const button = Array.from(c.container.querySelectorAll('button')).find(b => b.textContent?.startsWith('Interact:'))!
  button.click(); button.click(); key(c.surface, 'keydown', 'KeyE', { repeat: true }); advance(4)
  expect(c.intents.filter(i => i.buttons.includes('interact'))).toHaveLength(1)
})
it('fails safely when a prediction or intent consumer throws', async () => {
  const prediction = await controls({ onPrediction() { throw new Error('test') } }); expect(prediction.container.textContent).toContain('Movement unavailable'); expect(prediction.stick.disabled).toBe(true)
  const consume = await controls({ consume() { throw new Error('test') } }); consume.surface.focus(); key(consume.surface, 'keydown', 'KeyD'); advance(4)
  expect(consume.container.textContent).toContain('Movement unavailable'); expect(frames.size).toBe(0)
})
it('preserves later attribute replacements and removes all owned controls/listeners/frames/capture', async () => {
  const c = await controls(), remove = vi.spyOn(window, 'removeEventListener')
  c.surface.setAttribute('aria-label', 'Later application label'); eventPointer(c.stick, 'pointerdown', 1, 148, 100); advance(3)
  c.attachment.dispose(); c.attachment.dispose()
  expect(c.container.children).toHaveLength(0); expect(c.surface.getAttribute('aria-label')).toBe('Later application label'); expect(c.surface.hasAttribute('tabindex')).toBe(false)
  expect(c.surface.classList.contains('world-keyboard')).toBe(false); expect(frames.size).toBe(0); expect(c.captures.get(c.stick)?.size ?? 0).toBe(0)
  expect(remove.mock.calls.map(call => call[0])).toEqual(expect.arrayContaining(['keydown', 'keyup', 'pointermove', 'pointerup', 'pointercancel', 'blur', 'focus']))
})
it('camera follow, look-away, zoom and recenter remain local and stop after disposal', () => {
  const surface = document.createElement('canvas'), view = cameraFor(surface), camera = createCameraControl(view, { x: 1, y: 2 }, 2)
  camera.update({ x: 3, y: 4 }); expect(view.center).toHaveBeenLastCalledWith({ x: 3, y: 4 }, 2)
  camera.lookAt({ x: 90, y: 90 }); camera.update({ x: 5, y: 6 }); expect(view.center).toHaveBeenLastCalledWith({ x: 90, y: 90 }, 2)
  expect(camera.setZoom(NaN)).toBe(false); camera.setZoom(3); expect(view.center).toHaveBeenLastCalledWith({ x: 90, y: 90 }, 3)
  camera.recenter(); expect(view.center).toHaveBeenLastCalledWith({ x: 5, y: 6 }, 3)
  camera.dispose(); vi.mocked(view.center).mockClear(); camera.update({ x: 8, y: 9 }); camera.recenter(); expect(view.center).not.toHaveBeenCalled(); expect(camera.project({ x: 1, y: 1 })).toBeNull()
})
async function runtimeWith(attach: AttachControls, factoryPatch?: Partial<ReturnType<EngineFactory>>) {
  const { map, dispose } = await mapData(), host = document.createElement('div'), canvas = document.createElement('canvas'), close = deferred<void>(), destroy = vi.fn(() => close.resolve())
  document.body.append(host)
  const factory = vi.fn<EngineFactory>(() => { host.append(canvas); return { ready: Promise.resolve(), closed: close.promise, resize() {}, setVisible() {}, destroy, camera: cameraFor(canvas), ...factoryPatch } })
  const runtime = createWorldRuntime({ host, load: async () => map, importEngine: async () => factory, attachControls: attach }); cleanup.push(() => runtime.dispose())
  return { runtime, dispose, destroy, factory }
}
it.each(['stop', 'dispose', 'restart'] as const)('fences reentrant attachment factory %s and cleans returned late controls', async action => {
  let runtime!: WorldRuntime, calls = 0, next: Promise<unknown> | undefined
  const dispose = vi.fn(), enabled = vi.fn()
  const attach: AttachControls = () => {
    if (++calls === 1) { if (action === 'dispose') runtime.dispose(); else { runtime.stop(); if (action === 'restart') next = runtime.start() } }
    return { dispose, setEnabled: enabled }
  }
  const owned = await runtimeWith(attach); runtime = owned.runtime; await runtime.start()
  if (action === 'restart') { await next; expect(runtime.read().phase).toBe('ready'); expect(calls).toBe(2) }
  else expect(runtime.read().phase).toBe(action === 'stop' ? 'stopped' : 'disposed')
  expect(dispose).toHaveBeenCalledTimes(1); expect(owned.destroy).toHaveBeenCalledTimes(1)
})
it.each(['stop', 'dispose', 'restart'] as const)('fences reentrant attachment enable %s before readiness', async action => {
  let runtime!: WorldRuntime, count = 0, next: Promise<unknown> | undefined; const phases: string[] = [], dispose = vi.fn()
  const attach: AttachControls = () => ({ dispose, setEnabled() { if (++count !== 1) return; if (action === 'dispose') runtime.dispose(); else { runtime.stop(); if (action === 'restart') next = runtime.start() } } })
  runtime = (await runtimeWith(attach)).runtime; runtime.subscribe(() => phases.push(runtime.read().phase)); await runtime.start()
  if (action === 'restart') { await next; expect(phases.filter(p => p === 'ready')).toHaveLength(1) }
  else { expect(phases).not.toContain('ready'); expect(runtime.read().phase).toBe(action === 'dispose' ? 'disposed' : 'stopped') }
  expect(dispose).toHaveBeenCalledTimes(1)
})
it('persists pre-start block/visibility and never attaches after late readiness cancellation', async () => {
  const enabled = vi.fn(), dispose = vi.fn(), attach = vi.fn(() => ({ setEnabled: enabled, dispose }))
  const first = await runtimeWith(attach); first.runtime.setInputBlocked(true); first.runtime.setVisible(false); await first.runtime.start()
  expect(enabled).toHaveBeenLastCalledWith(false); first.runtime.setVisible(true); expect(enabled).toHaveBeenLastCalledWith(false); first.runtime.setInputBlocked(false); expect(enabled).toHaveBeenLastCalledWith(true)
  first.runtime.stop(); await first.runtime.settled(); expect(dispose).toHaveBeenCalledOnce()
  const ready = deferred<void>(), late = await runtimeWith(attach, { ready: ready.promise }), pending = late.runtime.start()
  await vi.waitFor(() => expect(late.factory).toHaveBeenCalledOnce()); late.runtime.stop(); ready.resolve(); await pending
  expect(attach).toHaveBeenCalledTimes(1)
})

it.each(['stop', 'dispose', 'block'] as const)('never emits or schedules late work after reentrant prediction %s', async action => {
  let c: Awaited<ReturnType<typeof controls>> | undefined, first = true
  c = await controls({ onPrediction() { if (!c || !first) return; first = false; if (action === 'stop') c.abort.abort(); else if (action === 'dispose') c.attachment.dispose(); else c.attachment.setEnabled(false) } })
  c.surface.focus(); key(c.surface, 'keydown', 'KeyD'); advance(4)
  expect(c.intents.filter(i => i.axisX !== 0 || i.axisY !== 0)).toHaveLength(0); expect(frames.size).toBe(0)
})
it.each(['stop', 'dispose', 'block'] as const)('neutralizes a reentrant intent consumer %s without replay', async action => {
  let c: Awaited<ReturnType<typeof controls>> | undefined, calls = 0, neutral = 0
  c = await controls({ consume(intent) { if (intent.axisX || intent.axisY) { calls++; if (action === 'stop') c?.abort.abort(); else if (action === 'dispose') c?.attachment.dispose(); else c?.attachment.setEnabled(false) } else neutral++ } })
  c.surface.focus(); key(c.surface, 'keydown', 'KeyD'); advance(6)
  expect(calls).toBe(1); expect(neutral).toBe(1); expect(frames.size).toBe(0)
})
it('does not let an invalid RAF timestamp create unbounded catch-up or a permanent frame', async () => {
  const c = await controls(); c.surface.focus(); key(c.surface, 'keydown', 'KeyD')
  for (const callback of Array.from(frames.values())) { frames.clear(); callback(NaN) }
  expect(frames.size).toBe(0); expect(c.attachment.read().foot).toEqual({ x: 80, y: 112 })
})
it('discards background elapsed time and requires fresh held input after resume', async () => {
  const c = await controls(); c.surface.focus(); key(c.surface, 'keydown', 'KeyD'); advance(3)
  c.attachment.setEnabled(false); const paused = c.attachment.read().foot; clock += 120_000; c.attachment.setEnabled(true); advance(4)
  expect(c.attachment.read().foot).toEqual(paused)
  key(c.surface, 'keydown', 'KeyD'); advance(2); expect(c.attachment.read().foot.x - paused.x).toBeCloseTo(6)
})
it('does not call a destroyed engine visibility method after attachment reentrantly stops', async () => {
  let runtime!: WorldRuntime, stop = false; const visible = vi.fn()
  const attach: AttachControls = () => ({ dispose() {}, setEnabled() { if (stop) runtime.stop() } })
  runtime = (await runtimeWith(attach, { setVisible: visible })).runtime; await runtime.start(); const initial = visible.mock.calls.length
  stop = true; runtime.setVisible(false); await runtime.settled()
  expect(visible).toHaveBeenCalledTimes(initial); expect(runtime.read().phase).toBe('stopped')
})
it('cleans failed enable and throwing disposal without stranding the engine or map', async () => {
  const dispose = vi.fn(() => { throw new Error('dispose failure') }), owned = await runtimeWith(() => ({ setEnabled() { throw new Error('enable failure') }, dispose }))
  expect(await owned.runtime.start()).toMatchObject({ phase: 'error', reason: 'engine-failed' }); await owned.runtime.settled()
  expect(dispose).toHaveBeenCalledOnce(); expect(owned.destroy).toHaveBeenCalledOnce(); expect(owned.dispose).toHaveBeenCalledOnce()
})

it('produces axes/buttons accepted by the actual C05 schema consumer with fixture-only stream context', async () => {
  const model = createMovementModel((await mapData()).map, policy), frame = inputFixture.messages.find(row => row.message.type === 'input')!.message
  for (const axis of [[1, 1], [-1, -1], [.6, .8], [2, 3], [NaN, 1], [0, 0]] as const) {
    model.axis(axis[0], axis[1]); const intent = model.step()
    expect(() => validate('Input', { ...frame, ...intent })).not.toThrow()
  }
  model.tap({ x: 144, y: 112 }); while (model.active()) model.step(); model.interact()
  const intent = model.step(); expect(intent.buttons).toEqual(['interact']); expect(() => validate('Input', { ...frame, ...intent })).not.toThrow()
})

it.each([true, false])('honors the latest same-owner visibility update ending in %s', async finalVisible => {
  let runtime!: WorldRuntime, armed = false
  const visible = vi.fn(), enabled = vi.fn((value: boolean) => {
    if (armed && value !== finalVisible) { armed = false; runtime.setVisible(finalVisible) }
  })
  runtime = (await runtimeWith(() => ({ dispose() {}, setEnabled: enabled }), { setVisible: visible })).runtime
  await runtime.start(); visible.mockClear(); enabled.mockClear(); armed = true
  runtime.setVisible(!finalVisible)
  expect(enabled).toHaveBeenLastCalledWith(finalVisible)
  expect(visible.mock.calls.map(call => call[0])).toEqual([finalVisible])
  expect(runtime.read().phase).toBe('ready')
})
