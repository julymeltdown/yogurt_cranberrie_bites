import { createWorldRuntime, type WorldRuntime } from '../../runtime'
import { attachMovement, type MovementAttachment } from '../../input/controller'
import type { MovementIntent, MovementSnapshot } from '../../input/model'
import type { Point, WorldCamera } from '../../camera/port'
import { createAssetResources, loadMap } from '../../map/resources'
import { movementData } from './movement-data'
const fixture = await movementData(), resources = createAssetResources({ origin: location.origin, allow: fixture.allow })
document.body.innerHTML = `<main><header><h1>Move in the test world</h1><p>SYNTHETIC FE15 · terrain and a geometric predicted-foot marker only. No character artwork, venue, server position or real arrival.</p></header>
<div class="tools"><button id="start">Start local preview</button><button id="stop">Stop local preview</button><button id="mount">Unmount owner</button><button id="panel">Open blocking panel</button><button id="visibility">Hide world</button></div>
<p id="runtime" role="status" aria-label="Runtime status">stopped</p><div class="viewport"><div id="host"></div><div id="marker" aria-hidden="true" hidden></div></div><p id="foot" aria-label="Predicted foot">Predicted foot: unavailable</p><div id="controls"></div>
<section id="panel-body" aria-label="Blocking panel" hidden><h2>Panel input</h2><label>Message <input id="message" autocomplete="off"></label><label>Panel choice <select id="panel-select"><option>One</option><option>Two</option></select></label><div id="editable" contenteditable="true" role="textbox" aria-label="Editable note" tabindex="0">Type a note</div><button id="ordinary">Ordinary panel action</button></section><p id="count">Interaction intents: 0</p>
<style>*{box-sizing:border-box}body{margin:0;background:#f2eee5;color:#231e1b;font:16px/1.5 system-ui}main{max-width:60rem;margin:auto;padding:16px}h1{font-size:1.8rem;margin:0 0 8px}p{margin:10px 0;overflow-wrap:anywhere}.tools{display:flex;flex-wrap:wrap;gap:8px}button,input,select{min-width:44px;min-height:44px;font:inherit;color:inherit;border:1px solid #655b50;border-radius:4px;background:#fffaf0;padding:8px}button:focus-visible,input:focus-visible,select:focus-visible,[contenteditable]:focus-visible{outline:3px solid #873342;outline-offset:3px}.viewport{position:relative;height:44vh;min-height:180px;max-height:420px;overflow:hidden;background:#151019}#host{width:100%;height:100%}#host canvas{display:block;image-rendering:pixelated}#marker{position:absolute;width:10px;height:10px;border:2px solid #fff;background:#873342;transform:translate(-50%,-100%);pointer-events:none}#panel-body{padding:12px 0}#panel-body label{display:grid;gap:4px;margin:8px 0}#editable{min-height:44px;border:1px solid #655b50;padding:8px}#controls{min-width:0}</style></main>`
const byId = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T
const host = byId('host'), container = byId('controls'), marker = byId('marker'), status = byId('runtime')
let runtime: WorldRuntime | null = null, movement: MovementAttachment | null = null, view: WorldCamera | null = null
let mode = 'valid', blocked = false, visible = true, attached = 0, disposedAttachments = 0, generation = 0
let markerFrame: number | null = null, snapshot: MovementSnapshot | null = null
const predictions: Point[] = []
const intents: { input: MovementIntent; time: number }[] = [], publications: { phase: string; generation: number }[] = []
function draw() {
  markerFrame = null
  if (!snapshot || runtime?.read().phase !== 'ready' || !view) { marker.hidden = true; return }
  const point = view.unproject(snapshot.foot), rect = host.getBoundingClientRect()
  marker.hidden = !point || point.x < rect.left || point.x > rect.right || point.y < rect.top || point.y > rect.bottom
  if (point) { marker.style.left = `${point.x - rect.left}px`; marker.style.top = `${point.y - rect.top}px` }
}
function scheduleMarker() { if (markerFrame === null) markerFrame = requestAnimationFrame(draw) }
function createOwner() {
  ++generation
  const owner = createWorldRuntime({ host,
    load: async signal => {
      const map = structuredClone(fixture.map)
      if (mode === 'invalid-map') map.geometry.spawns[0]!.cell_x = 0
      return loadMap({ mapText: JSON.stringify(map), themeText: JSON.stringify(fixture.theme), pin: fixture.pin, resources }, signal)
    },
    attachControls(context) {
      view = context.camera
      const controls = attachMovement(context, { controls: container, zoom: 1,
        policy: { speedPxPerSecond: mode === 'invalid-policy' ? 0 : 120, footWidthPx: 8, footHeightPx: 6, interactionRadiusPx: 42 },
        consume(input) {
          intents.push({ input, time: performance.now() }); byId('count').textContent = `Interaction intents: ${intents.filter(item => item.input.buttons.length).length}`
          if (mode.startsWith('consumer-') && (input.axisX !== 0 || input.axisY !== 0)) {
            const action = mode; mode = 'valid'
            if (action === 'consumer-dispose') owner.dispose()
            else { owner.stop(); if (action === 'consumer-restart') void owner.start() }
          }
        },
        onPrediction(value) { snapshot = value; predictions.push(value.foot); if (predictions.length > 512) predictions.shift(); byId('foot').textContent = `Predicted foot: ${value.foot.x.toFixed(2)}, ${value.foot.y.toFixed(2)} · local only`; scheduleMarker() },
      })
      attached++; movement = controls
      const observer = new ResizeObserver(scheduleMarker); observer.observe(host)
      return { setEnabled(value) {
        controls.setEnabled(value)
        if ((mode === 'visibility-show' && !value) || (mode === 'visibility-hide' && value)) {
          const next = mode === 'visibility-show'; mode = 'valid'; owner.setVisible(next)
        }
      }, dispose() {
        disposedAttachments++; observer.disconnect(); controls.dispose(); if (movement === controls) { movement = null; view = null }
        if (markerFrame !== null) { cancelAnimationFrame(markerFrame); markerFrame = null }; marker.hidden = true
      } }
    },
  })
  owner.setInputBlocked(blocked); owner.setVisible(visible)
  owner.subscribe(() => { if (['error', 'unavailable', 'disposed', 'stopped'].includes(owner.read().phase)) { view = null; snapshot = null }
    status.textContent = owner.read().phase + (owner.read().reason ? `: ${owner.read().reason}` : ''); publications.push({ phase: owner.read().phase, generation }); scheduleMarker() })
  runtime = owner; status.textContent = owner.read().phase
}
function mount(value: boolean) {
  if (!value) { runtime?.dispose(); runtime = null; movement = null; view = null; status.textContent = 'unmounted' }
  else if (!runtime) createOwner()
  byId('mount').textContent = runtime ? 'Unmount owner' : 'Mount owner'
}
createOwner()
byId('start').onclick = () => { void runtime?.start() }; byId('stop').onclick = () => runtime?.stop()
byId('mount').onclick = () => mount(!runtime)
byId('panel').onclick = () => {
  blocked = !blocked; runtime?.setInputBlocked(blocked); byId('panel-body').hidden = !blocked; byId('panel').textContent = blocked ? 'Close blocking panel' : 'Open blocking panel'
  if (blocked) byId('message').focus()
}
byId('visibility').onclick = () => { visible = !visible; runtime?.setVisible(visible); byId('visibility').textContent = visible ? 'Hide world' : 'Show world' }
declare global { interface Window { __FE15__: {
  state(): { runtime: ReturnType<WorldRuntime['read']> | null; movement: MovementSnapshot | null; camera: ReturnType<WorldCamera['read']> | null; intents: typeof intents; attached: number; disposedAttachments: number; publications: typeof publications; predictions: typeof predictions }
  start(): Promise<unknown> | undefined; stop(): void; settled(): Promise<void> | undefined; mount(value: boolean): void; mode(value: string): void
  project(point: Point): Point | null; roundtrip(point: Point): Point | null; lookAt(point: Point): void; blocked(value: boolean): void; visible(value: boolean): void
} } }
window.__FE15__ = { state: () => ({ runtime: runtime?.read() ?? null, movement: movement?.read() ?? null, camera: view?.read() ?? null, intents, attached, disposedAttachments, publications, predictions }),
  start: () => runtime?.start(), stop: () => runtime?.stop(), settled: () => runtime?.settled(), mount, mode(value) { mode = value },
  project: point => view?.unproject(point) ?? null, roundtrip: point => view?.project(point) ?? null,
  visible(value) { runtime?.setVisible(value) },
  lookAt(point) { movement?.camera.lookAt(point); scheduleMarker() }, blocked(value) { blocked = value; runtime?.setInputBlocked(value) },
}
