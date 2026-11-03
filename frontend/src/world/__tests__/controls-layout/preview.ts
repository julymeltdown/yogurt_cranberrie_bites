import { createWorldRuntime } from '../../runtime'
import { attachMovement, type MovementAttachment } from '../../input/controller'
import type { MovementIntent } from '../../input/model'
import { loadMap } from '../../map/resources'
import { movementData } from '../fixtures/movement-data'
// TEST ONLY actual Phaser terrain and original FE15 local model; no production art/server authority.
const fixture = await movementData(), host = document.getElementById('host')!, controls = document.getElementById('controls')!
const style = document.createElement('style'); style.textContent = 'body{margin:0;padding:12px;background:#f2eee5;color:#231e1b;font:16px/1.4 system-ui}h1{font-size:18px}#host{height:260px;width:100%}canvas{display:block}'; document.head.append(style)
let movement: MovementAttachment | null = null, failure = false
const intents: MovementIntent[] = []
const runtime = createWorldRuntime({ host, load: signal => loadMap({ mapText: JSON.stringify(fixture.map), themeText: JSON.stringify(fixture.theme), pin: fixture.pin,
  resources: { read: async r => r.kind === 'png' ? fixture.png : fixture.tiledBytes } }, signal),
  attachControls(context) { const attached = attachMovement(context, { controls, zoom: 1, policy: { speedPxPerSecond: 120, footWidthPx: 8, footHeightPx: 6, interactionRadiusPx: 42 },
    consume(intent) { intents.push(intent); if (failure && (intent.axisX || intent.axisY)) throw new Error('TEST ONLY consumer outage') } }); movement = attached
    return { setEnabled: attached.setEnabled, dispose() { attached.dispose(); movement = null } }
  },
})
const api = { state: () => ({ runtime: runtime.read(), movement: movement?.read(), intents }), blocked: (value: boolean) => runtime.setInputBlocked(value), fail: () => { failure = true },
  async dispose() { runtime.dispose(); await runtime.settled() } }
Object.assign(window, { __LAYOUT__: api })
await runtime.start()
