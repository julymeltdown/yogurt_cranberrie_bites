import type PhaserType from 'phaser'
import { useLayoutEffect, useRef, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { createWorldRuntime, type WorldRuntime } from '../../../world/runtime'
import { attachMovement, type MovementAttachment } from '../../../world/input/controller'
import { loadPhaserEngine } from '../../../world/scenes/MapScene'
import { createAssetResources } from '../../../world/map/resources'
import { createThemeLoader, type ThemeRelease } from '../../../world/theme/loader'
import { rehash } from '../../../world/__tests__/fixtures/synthetic'
import { syntheticRelease } from '../../../world/theme/__tests__/fixtures/releases'
import { themeIds, createLocalThemePreferences, type ThemeId } from '../model/preferences'
import { createThemeController, type ThemeController } from '../model/controller'
import { ThemeSelector } from '../ui/ThemeSelector'
import '../../../shared/styles/themes/themes.css'
const releases = {} as Record<ThemeId, ThemeRelease>
for (const id of themeIds) {
  const f = await syntheticRelease(id)
  releases[id] = { mapText: JSON.stringify(f.map), themeText: JSON.stringify(f.theme), pin: f.pin, resources: createAssetResources({ origin: location.origin, allow: f.allow }) }
}
const load = createThemeLoader(releases)
let runtime: WorldRuntime | null = null, controller: ThemeController | null = null, movement: MovementAttachment | null = null
let allocations = 0, attachments = 0, detachments = 0, mode = 'valid'
const textures = new Set<string>(), tilemaps = new Set<PhaserType.Tilemaps.Tilemap>()
const room = Object.freeze({ roomId: 'SYNTHETIC_ONLY', owner: 'one fixture connection sentinel' })
const avatar = Object.freeze({ userId: 'synthetic-user', config: 'source-art-not-loaded' })
const identity = { room, avatar }
const initialIdentity = identity
const Phaser = (await import('phaser')).default
const addImage = Phaser.Textures.TextureManager.prototype.addImage, removeTexture = Phaser.Textures.TextureManager.prototype.remove
Phaser.Textures.TextureManager.prototype.addImage = function(key, ...args) { const result = addImage.call(this, key, ...args); if (result && key.startsWith('mcr-reviewed-map-')) textures.add(key); return result }
Phaser.Textures.TextureManager.prototype.remove = function(key) { textures.delete(typeof key === 'string' ? key : key.key); return removeTexture.call(this, key) }
const blank = Phaser.Tilemaps.Tilemap.prototype.createBlankLayer
const destroyMap = Phaser.Tilemaps.Tilemap.prototype.destroy
Phaser.Tilemaps.Tilemap.prototype.destroy = function() { tilemaps.delete(this); return destroyMap.call(this) }
Phaser.Tilemaps.Tilemap.prototype.createBlankLayer = function(...args) { tilemaps.add(this); if (mode === 'engine-failure' && args[0] === 'overdraw') throw new Error('Synthetic preparation failure'); return blank.apply(this, args) }
function App() {
  const host = useRef<HTMLDivElement>(null), controls = useRef<HTMLDivElement>(null), [theme, setTheme] = useState<ThemeController | null>(null), [phase, setPhase] = useState('stopped'), [selected, setSelected] = useState<ThemeId>('REVENGE')
  useLayoutEffect(() => {
    const owner = createWorldRuntime({ host: host.current!, load: signal => load('REVENGE', signal), timeoutMs: 1000,
      importEngine: async () => { const factory = await loadPhaserEngine(); return (host, map) => { allocations++; return factory(host, map) } },
      attachControls(context) {
        attachments++
        const attached = attachMovement(context, { controls: controls.current!, zoom: 3, policy: { speedPxPerSecond: 70, footWidthPx: 8, footHeightPx: 6, interactionRadiusPx: 64 }, consume() {} })
        movement = attached
        return { setEnabled: value => attached.setEnabled(value), dispose() { detachments++; attached.dispose(); movement = null } }
      },
    })
    runtime = owner
    const choice = createThemeController({ runtime: owner, initial: 'REVENGE', scope: { userId: 'synthetic-user', eventId: 'synthetic-event' }, preferences: createLocalThemePreferences(localStorage), load: async (id, signal) => {
      const candidate = await load(id, signal)
      // Deliberately ignore abort AFTER decoding to test late-resource fencing.
      if (mode === 'slow' && id === 'PARADE') await new Promise(resolve => setTimeout(resolve, 350))
      if (mode === 'geometry-mismatch' || mode === 'avatar-mismatch' || mode === 'tilemap-binding') {
        const map = structuredClone(candidate.map), theme = structuredClone(candidate.theme)
        if (mode === 'geometry-mismatch') map.geometry.collision_grid[0]![0] = 0
        else if (mode === 'tilemap-binding') map.tilemap.path = 'world/fe06/changed.json'
        else theme.avatar_manifest_id = 'wrong_avatar'
        await rehash(map, theme)
        return { ...candidate, map, theme }
      }
      return candidate
    } })
    controller = choice; setTheme(choice)
    const unsubscribeTheme = choice.subscribe(() => setSelected(choice.read().selected))
    const unsubscribe = owner.subscribe(() => setPhase(owner.read().phase))
    void owner.start().then(() => choice.restore())
    return () => { unsubscribe(); unsubscribeTheme(); choice.dispose(); owner.dispose(); controller = null; runtime = null }
  }, [])
  const content = <main><h1>Theme system test</h1><p>SYNTHETIC FE06 · three test swatches per skin. No character artwork, production world, venue or real multiplayer.</p>
    {theme && <ThemeSelector controller={theme} />}<p aria-label="Runtime status">{phase}</p>
    <div ref={host} className="map-host" /><div ref={controls} />
    <button onClick={() => { controller?.dispose(); runtime?.dispose() }}>Dispose fixture</button>
    <style>{`*{box-sizing:border-box}body{margin:0;font:16px/1.5 system-ui}main{max-width:50rem;margin:auto;padding:12px}h1{font-size:1.6rem}button{min-width:44px;min-height:44px}.map-host{height:270px;width:100%;overflow:hidden}.map-host canvas{display:block;image-rendering:pixelated}`}</style>
  </main>
  return <div className="mcr-theme-scope" data-mcr-theme={selected}>{content}</div>
}
declare global { interface Window { __FE06__: {
  state(): unknown; select(id: ThemeId): Promise<void> | undefined; mode(value: string): void; settled(): Promise<void> | undefined
} } }
window.__FE06__ = {
  state: () => ({ theme: controller?.read(), runtime: runtime?.read(), skin: runtime?.readSkin(), camera: movement?.camera.read(), movement: movement?.read(), allocations, attachments, detachments, textures: [...textures], tilemaps: tilemaps.size, identityPreserved: identity === initialIdentity && identity.room === room && identity.avatar === avatar }),
  select: id => controller?.select(id), mode(value) { mode = value }, settled: () => runtime?.settled(),
}
createRoot(document.getElementById('root')!).render(<App />)
