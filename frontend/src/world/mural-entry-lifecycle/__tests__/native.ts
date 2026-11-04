import type Phaser from 'phaser'
import { createPhaserMuralScene } from '../../mural/phaser'
import { fixture } from './fixture'
if (!import.meta.env.DEV) throw Error('Synthetic lifecycle fixture is development-only')
const f = fixture(), key = 'fixture-only-wall', status = document.getElementById('status')!
let scene: Phaser.Scene | null = null, game: Phaser.Game | null = null, created = 0, disposed = false
const current = () => f.owner.read().sync
const render = () => { status.textContent = current() ? `Revision ${current()!.read().revision}; command ${current()!.editor.read().phase}` : 'Release unavailable' }
function qualify() {
  if (!scene || disposed) return
  f.setView({ ...f.view(), phase: 'live', visible: true })
  const r = f.qualify()
  f.lifecycle.bind({ ...r.input, textureKey: key, scene(anchor, textureKey) {
    const port = createPhaserMuralScene(scene!, anchor, textureKey)
    if (!port.onClose) throw Error('Missing incumbent close capability')
    return { ...port, onClose: port.onClose }
  } })
  current()?.subscribe(render); render()
}
f.lifecycle.subscribe(render)
document.getElementById('place')!.onclick = () => {
  f.server.mode('lost'); const sync = current(); sync?.editor.select(2, 3); sync?.editor.chooseColor(2)
  void sync?.editor.confirm().then(render)
}
document.getElementById('withdraw')!.onclick = () => { f.setView({ ...f.view(), phase: 'closed' }); render() }
document.getElementById('qualify')!.onclick = qualify
document.getElementById('check')!.onclick = () => { f.server.mode('ack'); f.server.resultAvailable(true); void current()?.editor.check().then(render) }
document.getElementById('policy')!.onclick = () => { f.auth.privacy.changePolicy(f.scope, 'block'); render() }
const { default: P } = await import('phaser')
game = new P.Game({ type: P.CANVAS, width: 176, height: 176, parent: document.getElementById('world')!, backgroundColor: '#201D1D',
  audio: { noAudio: true }, banner: false, render: { pixelArt: true }, scene: { create(this: Phaser.Scene) { scene = this.scene.get(this.sys.settings.key); qualify() } } })
++created
const probe = {
  read() {
    const sync = current(), held = f.owner.read().sync?.editor.read().command
    return { available: !!f.lifecycle.read(), revision: sync?.read().revision ?? null, phase: sync?.editor.read().phase ?? null,
      commandKey: held?.key ?? null, posts: f.requests.filter(r => r.method === 'POST').map(r => r.headers.get('Idempotency-Key')),
      texturePresent: !!scene?.textures.exists(key), objects: scene?.children.list.length ?? 0, created, games: game ? 1 : 0,
      worldListeners: f.worldCount(), timers: f.clock.count(), disposed: f.lifecycle.isDisposed() }
  },
  dispose() { if (disposed) return; disposed = true; f.dispose(); game?.destroy(true); game = null },
}
window.__MURAL_LIFECYCLE__ = probe
declare global { interface Window { __MURAL_LIFECYCLE__: typeof probe } }
