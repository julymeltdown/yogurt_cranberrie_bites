import type { ControlAttachment, ControlContext } from '../runtime'
import { MapError } from '../map/json'
import { createCameraControl } from '../camera/port'
import { createMovementModel, STEP_MS, type MovementIntent, type MovementSnapshot } from './model'
import type { SimulationPolicy } from './simulation'
import './controls.css'
export interface MovementOptions {
  readonly controls: HTMLElement
  readonly policy: SimulationPolicy
  readonly zoom: number
  readonly consume: (intent: MovementIntent) => void
  readonly onPrediction?: (snapshot: MovementSnapshot) => void
}
export interface MovementAttachment extends ControlAttachment {
  read(): MovementSnapshot
  camera: ReturnType<typeof createCameraControl>
}
/** DOM input owns listeners/capture/RAF. Phaser input plugins stay disabled. */
export function attachMovement(context: ControlContext, options: MovementOptions): MovementAttachment {
  const surface = context.camera.surface()
  if (!surface || context.signal.aborted) throw new MapError('invalid')
  let model: ReturnType<typeof createMovementModel>, camera: ReturnType<typeof createCameraControl>
  try { model = createMovementModel(context.map, options.policy); camera = createCameraControl(context.camera, model.read().foot, options.zoom) }
  catch { throw new MapError('invalid') }
  let enabled = false, disposed = false, failed = false, focused = true, epoch = 0, frame: number | null = null, lastTime: number | null = null, accumulated = 0
  let pointer: { id: number; kind: 'stick' | 'tap'; element: HTMLElement; x: number; y: number; time: number; dragged: boolean } | null = null
  let lastIntent: MovementIntent = { axisX: 0, axisY: 0, buttons: [] }
  const held = new Set<string>(), removers: (() => void)[] = []
  const root = document.createElement('section'); root.className = 'world-controls'; root.setAttribute('aria-label', 'World movement controls')
  const stick = document.createElement('button'); stick.type = 'button'; stick.className = 'world-joystick'; stick.setAttribute('aria-label', 'Movement joystick. Arrow keys or W A S D when focused')
  const knob = document.createElement('span'); knob.setAttribute('aria-hidden', 'true'); stick.append(knob)
  const actions = document.createElement('div'); actions.className = 'world-actions'
  const recenter = document.createElement('button'); recenter.type = 'button'; recenter.textContent = 'Recenter'
  const zoomIn = document.createElement('button'); zoomIn.type = 'button'; zoomIn.textContent = 'Zoom +'; zoomIn.setAttribute('aria-label', 'Zoom in')
  const zoomOut = document.createElement('button'); zoomOut.type = 'button'; zoomOut.textContent = 'Zoom −'; zoomOut.setAttribute('aria-label', 'Zoom out')
  const interact = document.createElement('button'); interact.type = 'button'; interact.textContent = 'Interact'
  const label = document.createElement('label'); label.append('Nearby in world')
  const select = document.createElement('select'); select.setAttribute('aria-label', 'Nearby interaction'); label.append(select)
  const hint = document.createElement('p'); hint.textContent = 'Focus the map or joystick: arrows / W A S D move, E interacts, R recenters. Tap a walkable destination. Virtual movement never confirms real arrival.'
  const status = document.createElement('p'); status.setAttribute('role', 'status'); status.setAttribute('aria-live', 'polite')
  actions.append(recenter, zoomIn, zoomOut, interact, label); root.append(stick, actions, hint, status); options.controls.append(root)
  const attributes = new Map(['tabindex', 'role', 'aria-label'].map(key => [key, surface.getAttribute(key)]))
  const ownedAttributes = { tabindex: '0', role: 'group', 'aria-label': 'World movement map. Arrow keys or W A S D move. E interacts. R recenters.' }
  for (const [key, value] of Object.entries(ownedAttributes)) surface.setAttribute(key, value)
  const hadClass = surface.classList.contains('world-keyboard'); surface.classList.add('world-keyboard')
  function listen(target: EventTarget, type: string, listener: EventListener, options?: AddEventListenerOptions) {
    target.addEventListener(type, listener, options); removers.push(() => target.removeEventListener(type, listener, options))
  }
  const running = () => !disposed && !failed && enabled && focused && !document.hidden && !context.signal.aborted
  const activeIntent = (intent: MovementIntent) => intent.axisX !== 0 || intent.axisY !== 0 || intent.buttons.length > 0
  function emit(intent: MovementIntent) {
    if (!activeIntent(intent) && !activeIntent(lastIntent)) return
    intent = Object.freeze({ axisX: intent.axisX, axisY: intent.axisY, buttons: Object.freeze(Array.from(intent.buttons)) })
    lastIntent = intent // Reentrant consumers cannot replay the previous one-shot or recursively neutralize it.
    try { options.consume(intent) } catch { failed = true; enabled = false; clear(); status.textContent = 'Movement unavailable: input consumer failed.' }
  }
  function releasePointer() {
    const old = pointer; pointer = null
    if (old) { try { if (old.element.hasPointerCapture(old.id)) old.element.releasePointerCapture(old.id) } catch { /* Detached/cancelled capture is already released. */ } }
    stick.style.removeProperty('--stick-x'); stick.style.removeProperty('--stick-y')
  }
  function clear() {
    ++epoch; held.clear(); model.reset(); releasePointer(); lastTime = null; accumulated = 0
    if (frame !== null) { cancelAnimationFrame(frame); frame = null }
    emit({ axisX: 0, axisY: 0, buttons: [] })
  }
  function refresh() {
    const snapshot = model.read(), selected = snapshot.nearby.find(target => target.interactionId === snapshot.selectedId)
    const entries = snapshot.nearby.map(target => ({ id: target.interactionId, label: target.label }))
    const current = Array.from(select.options).map(option => ({ id: option.value, label: option.textContent }))
    if (JSON.stringify(entries) !== JSON.stringify(current)) {
      select.replaceChildren()
      if (!entries.length) { const option = document.createElement('option'); option.value = ''; option.textContent = 'No reachable interaction nearby'; select.append(option) }
      else for (const entry of entries) { const option = document.createElement('option'); option.value = entry.id; option.textContent = entry.label; select.append(option) }
    }
    select.value = selected?.interactionId ?? ''; select.disabled = !running() || !entries.length
    interact.disabled = !running() || !selected; interact.textContent = selected ? `Interact: ${selected.label}` : 'Interact'
    stick.disabled = !running(); recenter.disabled = !running(); zoomIn.disabled = !running(); zoomOut.disabled = !running()
    if (!disposed && !failed && !context.signal.aborted) {
      try { camera.update(snapshot.foot); if (!disposed && !context.signal.aborted) options.onPrediction?.(snapshot) }
      catch { failed = true; enabled = false; clear(); status.textContent = 'Movement unavailable: prediction consumer failed.' }
    }
  }
  function schedule() { if (running() && frame === null && (model.active() || activeIntent(lastIntent))) frame = requestAnimationFrame(tick) }
  function tick(time: number) {
    frame = null
    if (!Number.isFinite(time)) { clear(); return }
    if (!running()) { clear(); return }
    const ticket = epoch
    if (lastTime !== null) accumulated += Math.max(0, Math.min(100, time - lastTime))
    lastTime = time
    while (accumulated >= STEP_MS) {
      accumulated -= STEP_MS
      const intent = model.step(); refresh()
      if (!running() || ticket !== epoch) return
      emit(intent)
      if (!running() || ticket !== epoch) return
    }
    schedule()
  }
  const keyboardTarget = (target: EventTarget | null) => target instanceof HTMLElement && (target === surface || target === stick) && document.activeElement === target
    && !target.closest('input,textarea,select,[contenteditable]:not([contenteditable="false"]),[role="textbox"],dialog,[role="dialog"]')
  const directions: Record<string, readonly [number, number]> = { ArrowLeft: [-1, 0], KeyA: [-1, 0], ArrowRight: [1, 0], KeyD: [1, 0], ArrowUp: [0, -1], KeyW: [0, -1], ArrowDown: [0, 1], KeyS: [0, 1] }
  const updateKeys = () => { let x = 0, y = 0; for (const code of held) { const value = directions[code]!; x += value[0]; y += value[1] } model.axis(x, y); schedule() }
  listen(window, 'keydown', event => {
    const e = event as KeyboardEvent
    if (!running() || !keyboardTarget(e.target) || e.defaultPrevented || e.isComposing || e.keyCode === 229 || e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) { clear(); return }
    if (Object.hasOwn(directions, e.code)) { e.preventDefault(); if (!e.repeat && !pointer) { held.add(e.code); updateKeys() } }
    else if (e.code === 'KeyE') { e.preventDefault(); if (!e.repeat && model.interact()) schedule() }
    else if (e.code === 'KeyR') { e.preventDefault(); if (!e.repeat) camera.recenter() }
    else if (e.code === 'Escape') { e.preventDefault(); clear() }
  })
  listen(window, 'keyup', event => { const e = event as KeyboardEvent; if (held.delete(e.code)) { if (keyboardTarget(e.target)) e.preventDefault(); updateKeys() } })
  function down(kind: 'stick' | 'tap', e: PointerEvent, element: HTMLElement) {
    if (!running() || pointer || e.button !== 0 || !Number.isFinite(e.clientX) || !Number.isFinite(e.clientY)) return
    e.preventDefault(); clear(); element.focus({ preventScroll: true })
    if (!running()) return
    pointer = { id: e.pointerId, kind, element, x: e.clientX, y: e.clientY, time: e.timeStamp, dragged: false }
    try { element.setPointerCapture(e.pointerId) } catch { clear(); return }
    if (kind === 'stick') moveStick(e)
  }
  function moveStick(e: PointerEvent) {
    const rect = stick.getBoundingClientRect(), radius = Math.min(rect.width, rect.height) / 2, dx = e.clientX - (rect.left + rect.width / 2), dy = e.clientY - (rect.top + rect.height / 2)
    if (!(radius > 0) || !Number.isFinite(dx) || !Number.isFinite(dy)) { clear(); return }
    const distance = Math.hypot(dx, dy), strength = Math.max(0, Math.min(1, (distance / radius - .12) / .88)), divisor = Math.max(1, distance)
    model.axis(dx / divisor * strength, dy / divisor * strength)
    stick.style.setProperty('--stick-x', `${dx / divisor * strength * radius * .5}px`); stick.style.setProperty('--stick-y', `${dy / divisor * strength * radius * .5}px`); schedule()
  }
  listen(stick, 'pointerdown', event => down('stick', event as PointerEvent, stick))
  listen(surface, 'pointerdown', event => down('tap', event as PointerEvent, surface))
  listen(window, 'pointermove', event => {
    const e = event as PointerEvent
    if (!pointer || pointer.id !== e.pointerId || !running()) return
    if (pointer.kind === 'stick') moveStick(e)
    else if (Math.hypot(e.clientX - pointer.x, e.clientY - pointer.y) > 10) pointer.dragged = true
  })
  listen(window, 'pointerup', event => {
    const e = event as PointerEvent, old = pointer
    if (!old || old.id !== e.pointerId) return
    releasePointer()
    if (old.kind === 'stick') { clear(); return }
    if (running() && !old.dragged && Math.hypot(e.clientX - old.x, e.clientY - old.y) <= 10 && e.timeStamp >= old.time && e.timeStamp - old.time <= 500) {
      const point = camera.project({ x: e.clientX, y: e.clientY })
      if (!point || !model.tap(point)) status.textContent = 'Choose a reachable, walkable destination.'
      else { status.textContent = 'Moving locally. Server position remains authoritative.'; schedule() }
    }
  })
  const cancelled: EventListener = event => { if (pointer?.id === (event as PointerEvent).pointerId) clear() }
  listen(window, 'pointercancel', cancelled); listen(surface, 'lostpointercapture', cancelled); listen(stick, 'lostpointercapture', cancelled)
  listen(window, 'blur', () => { focused = false; clear(); refresh() })
  listen(window, 'focus', () => { focused = true; clear(); refresh() })
  listen(document, 'visibilitychange', () => { clear(); refresh() })
  listen(document, 'focusin', event => { if (!keyboardTarget(event.target)) clear() })
  listen(document, 'pointerdown', event => { if (event.target !== surface && event.target !== stick && event.target !== knob) clear() }, { capture: true })
  listen(interact, 'click', () => { if (running() && model.interact()) schedule() })
  listen(interact, 'keydown', event => { if ((event as KeyboardEvent).repeat) event.preventDefault() })
  listen(recenter, 'click', () => { if (running()) { camera.recenter(); refresh() } })
  listen(zoomIn, 'click', () => { if (running()) { camera.setZoom(Math.min(8, (camera.read().camera?.zoom ?? options.zoom) + 1)); refresh() } })
  listen(zoomOut, 'click', () => { if (running()) { camera.setZoom(Math.max(1, (camera.read().camera?.zoom ?? options.zoom) - 1)); refresh() } })
  listen(select, 'change', () => { clear(); model.select(select.value); refresh() })
  listen(context.signal, 'abort', () => { enabled = false; clear(); refresh() }, { once: true })
  refresh()
  return {
    camera, read: () => model.read(),
    setEnabled(value) { if (disposed) return; enabled = value; clear(); refresh(); if (!failed) status.textContent = running() ? 'Local movement ready.' : 'Movement paused.' },
    dispose() {
      if (disposed) return
      disposed = true; enabled = false; clear(); for (const remove of removers.splice(0)) remove(); camera.dispose(); root.remove()
      for (const [key, value] of Object.entries(ownedAttributes)) if (surface.getAttribute(key) === value) { const prior = attributes.get(key); if (prior === null || prior === undefined) surface.removeAttribute(key); else surface.setAttribute(key, prior) }
      if (!hadClass) surface.classList.remove('world-keyboard')
    },
  }
}
