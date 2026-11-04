import { createEnginePreferenceEffects, type EnginePreferenceEffects } from '../preferences/engine'
import { createPhaserMuralScene } from '../mural/phaser'
import type { MuralSceneTexture } from '../mural/sink'
import type { IncumbentMuralScene } from '../runtime'
import type { Point, WorldCamera } from '../camera/port'
import type PhaserType from 'phaser'
import type { EngineFactory } from '../runtime'
import type { LoadedMap } from '../map/resources'
import { assertSameWorld } from '../theme/compatibility'
import type { AvatarScenePort, AvatarSceneTexture, AvatarSceneSprite } from '../avatar/scene'
import { prepareIllustratedSkin } from '../illustrated/phaser'
import { validateLoadedScenery } from '../illustrated/model'
import { referenceSceneFor } from '../reference-scene/model'
import { spatialV5CameraPresentation } from '../reference-scene/spatial-v5/camera'
import { prepareReferenceSceneSkin } from '../reference-scene/phaser'
/** Phaser is evaluated only after an explicitly started owner has loaded and verified resources. */
export async function loadPhaserEngine(): Promise<EngineFactory> {
  const Phaser = (await import('phaser')).default
  return (host, loaded) => {
    let stopped = false, destroyed = false, visible = true
    let resolveReady!: () => void, rejectReady!: (reason: unknown) => void, resolveClosed!: () => void
    const ready = new Promise<void>((resolve, reject) => { resolveReady = resolve; rejectReady = reject })
    const closed = new Promise<void>(resolve => { resolveClosed = resolve })
    const g = loaded.map.geometry
    const reference = referenceSceneFor(loaded)
    const muralClose = new Set<() => void>()
    const closeMural = () => { for (const close of Array.from(muralClose)) { try { close() } catch { /* Continue scene teardown. */ } }; muralClose.clear() }
    let skinRevision = 0
    let peakTextures = 0, peakDecodedBytes = 0
    const readScenery = () => {
      const keys = destroyed ? [] : game.textures.getTextureKeys().filter(key => key.startsWith('mcr-reviewed-map-'))
      const decodedBytes = keys.reduce((n, key) => { const image = game.textures.get(key).getSourceImage() as HTMLImageElement; return n + image.width * image.height * 4 }, 0)
      peakTextures = Math.max(peakTextures, keys.length); peakDecodedBytes = Math.max(peakDecodedBytes, decodedBytes)
      const objects = !sceneReady || destroyed ? 0 : game.scene.getScene('mcr-map').children.list.filter(child => child instanceof Phaser.GameObjects.Image && child.texture.key.startsWith('mcr-reviewed-map-')).length
      return Object.freeze({ textures: keys.length, decodedBytes, objects, peakTextures, peakDecodedBytes })
    }
    type Skin = { show(): void; remove(): void }
    let skin: Skin | null = null, effects: EnginePreferenceEffects | null = null
    let sceneReady = false, cameraFoot: Point | null = null, cameraZoom: number | null = null
    let avatarRevision = 0
    const avatarTextures = new Map<AvatarSceneTexture, () => void>(), avatarSprites = new Map<AvatarSceneSprite, () => void>(), avatarListeners = new Set<() => void>()
    const clearAvatars = () => {
      for (const remove of Array.from(avatarListeners)) remove()
      for (const remove of Array.from(avatarSprites.values())) remove()
      for (const remove of Array.from(avatarTextures.values())) remove()
    }
    class MapScene extends Phaser.Scene {
      constructor() { super('mcr-map') }
      create() {
        try {
          if (stopped) return
          sceneReady = true
          skin = this.prepareSkin(loaded)
          skin.show()
          effects = createEnginePreferenceEffects(game.renderer as PhaserType.Renderer.Canvas.CanvasRenderer, this.tweens)
          const remove = () => { effects?.dispose(); effects = null; sceneReady = false; closeMural(); clearAvatars(); skin?.remove(); skin = null }
          this.events.once(Phaser.Scenes.Events.SHUTDOWN, remove)
          this.events.once(Phaser.Scenes.Events.DESTROY, remove)
          // Phaser Canvas roundPixels expands every frame by half a source pixel.
          // Illustrated atlas rectangles must keep their exact placement/extents instead.
          this.cameras.main.setBounds(0, 0, loaded.widthPx, loaded.heightPx).setRoundPixels(!loaded.scenery)
          this.cameras.main.setBackgroundColor(loaded.theme.palette.background)
          resolveReady()
        } catch (error) { rejectReady(error) }
      }
      /** Prepare hidden layers before touching the active renderer. No camera/input/network mutation. */
      prepareSkin(next: LoadedMap): Skin {
        const textureKey = `mcr-reviewed-map-${++skinRevision}`
        validateLoadedScenery(next)
        if (referenceSceneFor(next)) return prepareReferenceSceneSkin(this, next, textureKey, Phaser.Textures.FilterMode.NEAREST, () => { readScenery() }, () => !stopped && sceneReady)
        if (next.scenery) return prepareIllustratedSkin(this, next, textureKey, Phaser.Textures.FilterMode.NEAREST, () => { readScenery() }, () => !stopped && sceneReady)
        let tilemap: PhaserType.Tilemaps.Tilemap | null = null, removed = false
        const layers: PhaserType.Tilemaps.TilemapLayer[] = []
        const remove = () => {
          if (removed) return; removed = true
          tilemap?.destroy()
          if (this.textures.exists(textureKey)) this.textures.remove(textureKey)
        }
        try {
          const texture = this.textures.addImage(textureKey, next.image)
          if (!texture) throw new Error('Map texture unavailable')
          texture.setFilter(Phaser.Textures.FilterMode.NEAREST)
          tilemap = this.make.tilemap({ tileWidth: g.tile_width_px, tileHeight: g.tile_height_px, width: g.width_cells, height: g.height_cells })
          const tiles = tilemap.addTilesetImage('theme', textureKey, g.tile_width_px, g.tile_height_px, 0, 0, 0)
          if (!tiles) throw new Error('Map tiles unavailable')
          for (const [name, grid, depth] of [['terrain', g.terrain_layer, 0], ['overdraw', g.overdraw_layer, 2]] as const) {
            const layer = tilemap.createBlankLayer(name, tiles)
            if (!layer) throw new Error('Map layer unavailable')
            layers.push(layer)
            layer.setVisible(false).putTilesAt(grid.map(row => row.map(id => id === 0 ? -1 : id - 1)), 0, 0).setDepth(depth)
          }
          return { show: () => { for (const layer of layers) layer.setVisible(true) }, remove }
        } catch (error) { remove(); throw error }
      }
    }
    // 3.90.0 VisibilityHandler has no teardown and assigns window properties. Scope its
    // synchronous registration; do not retain a process-wide EventTarget patch.
    const visibilityHandlers: { listener: EventListenerOrEventListenerObject; capture: boolean }[] = []
    let priorBlur: typeof window.onblur = null, priorFocus: typeof window.onfocus = null
    let ownedBlur: typeof window.onblur = null, ownedFocus: typeof window.onfocus = null
    class OwnedGame extends Phaser.Game {
      start() {
        priorBlur = window.onblur; priorFocus = window.onfocus
        const add = document.addEventListener, hadOwn = Object.hasOwn(document, 'addEventListener')
        document.addEventListener = function(this: Document, type: string, listener: EventListenerOrEventListenerObject, options?: boolean | AddEventListenerOptions) {
          if (type === 'visibilitychange') visibilityHandlers.push({ listener, capture: typeof options === 'boolean' ? options : options?.capture === true })
          return add.call(this, type, listener, options)
        } as typeof document.addEventListener
        try { super.start() }
        finally { if (hadOwn) document.addEventListener = add; else Reflect.deleteProperty(document, 'addEventListener'); ownedBlur = window.onblur; ownedFocus = window.onfocus }
      }
    }
    const game = new OwnedGame({ type: Phaser.CANVAS, parent: host, width: 1, height: 1, scene: [MapScene],
      banner: false, autoFocus: false, pixelArt: true, antialias: false, roundPixels: true, transparent: false,
      audio: { noAudio: true }, input: { keyboard: false, mouse: false, touch: false, gamepad: false },
      scale: { mode: Phaser.Scale.NONE, autoCenter: Phaser.Scale.NO_CENTER, autoRound: true },
      callbacks: { postBoot: () => { queueMicrotask(() => { if (stopped) flushDestroy() }) } },
    })
    game.events.once(Phaser.Core.Events.DESTROY, () => { destroyed = true; sceneReady = false
      for (const { listener, capture } of visibilityHandlers) document.removeEventListener('visibilitychange', listener, capture)
      visibilityHandlers.length = 0
      if (window.onblur === ownedBlur) window.onblur = priorBlur
      if (window.onfocus === ownedFocus) window.onfocus = priorFocus
      const debugWindow = window as unknown as { PHASER_GAME?: PhaserType.Game }
      if (debugWindow.PHASER_GAME === game) delete debugWindow.PHASER_GAME
      resolveClosed() })
    function flushDestroy() {
      if (destroyed || !game.isRunning) return
      // 3.90.0 public destroy() defers to the next step. Flush that pending step even while asleep.
      // This avoids private runDestroy(), a stranded RAF, or overlap with the next owned game.
      game.destroy(true, false); game.step(performance.now(), 0)
    }
    function refreshCamera() {
      if (stopped || !sceneReady) return
      const camera = game.scene.getScene('mcr-map').cameras.main
      camera.setSize(game.scale.width, game.scale.height).setZoom(cameraZoom ?? Math.max(1, Math.floor(Math.min(game.scale.width / loaded.widthPx, game.scale.height / loaded.heightPx))))
      // Before a server/self or explicit camera target exists, present the bound stage.
      // This is a view only: no spawn, player, geometry or interaction is moved.
      const stage = loaded.scenery?.manifest.objects.find(object => object.id === 'stage')
      const stageInteraction = g.interactions.find(interaction => interaction.interaction_id === 'stage_info')
      const foot = cameraFoot ?? (reference ? { x: reference.worldCamera.x + reference.worldCamera.width / 2, y: reference.worldCamera.y + reference.worldCamera.height / 2 }
        : stage && stageInteraction ? { x: stage.x + stage.width / 2 + Math.min(64, game.scale.width / camera.zoom * .14),
        y: stage.y + stage.height / 2 + Math.min(260, game.scale.height / camera.zoom * .32) }
        : { x: (loaded.spawn.cell_x + .5) * g.tile_width_px, y: (loaded.spawn.cell_y + .5) * g.tile_height_px })
      const presentation = spatialV5CameraPresentation(loaded, { width: game.scale.width, height: game.scale.height }, foot, cameraZoom ?? 1)
      if (presentation) camera.setZoom(presentation.zoom)
      camera.centerOn(presentation?.center.x ?? foot.x, presentation?.center.y ?? foot.y)
      camera.preRender() // Updates bounded scroll and the matrix used by public getWorldPoint().
    }
    const cameraView: WorldCamera = {
      surface: () => stopped ? null : game.canvas,
      center(foot, zoom) {
        if (stopped || !sceneReady || !Number.isFinite(foot.x) || !Number.isFinite(foot.y) || !Number.isFinite(zoom) || zoom < 1 || zoom > 8) return
        cameraFoot = { x: Math.max(0, Math.min(loaded.widthPx, foot.x)), y: Math.max(0, Math.min(loaded.heightPx, foot.y)) }; cameraZoom = zoom; refreshCamera()
      },
      project(client) {
        if (stopped || !sceneReady || !Number.isFinite(client.x) || !Number.isFinite(client.y)) return null
        const rect = game.canvas.getBoundingClientRect()
        if (rect.width <= 0 || rect.height <= 0 || client.x < rect.left || client.y < rect.top || client.x >= rect.right || client.y >= rect.bottom) return null
        refreshCamera()
        const point = game.scene.getScene('mcr-map').cameras.main.getWorldPoint((client.x - rect.left) * game.canvas.width / rect.width, (client.y - rect.top) * game.canvas.height / rect.height)
        return point.x < 0 || point.y < 0 || point.x > loaded.widthPx || point.y > loaded.heightPx ? null : { x: point.x, y: point.y }
      },
      unproject(world) {
        if (stopped || !sceneReady || !Number.isFinite(world.x) || !Number.isFinite(world.y)) return null
        refreshCamera()
        const camera = game.scene.getScene('mcr-map').cameras.main, origin = camera.getWorldPoint(0, 0), end = camera.getWorldPoint(game.canvas.width, game.canvas.height), rect = game.canvas.getBoundingClientRect()
        if (rect.width <= 0 || rect.height <= 0) return null
        return { x: rect.left + (world.x - origin.x) / (end.x - origin.x) * rect.width, y: rect.top + (world.y - origin.y) / (end.y - origin.y) * rect.height }
      },
      read() {
        if (stopped || !sceneReady) return null
        refreshCamera()
        const camera = game.scene.getScene('mcr-map').cameras.main
        return { zoom: camera.zoom, scrollX: camera.scrollX, scrollY: camera.scrollY, width: camera.width, height: camera.height }
      },
    }
    const avatarTextureFrames = new WeakMap<AvatarSceneTexture, number>()
    function createAvatarTexture(canvas: HTMLCanvasElement, frames: 1 | 20): AvatarSceneTexture {
      if (stopped || !sceneReady || canvas.width !== (frames === 1 ? 64 : 320) || canvas.height !== (frames === 1 ? 64 : 256)) throw new Error('Avatar scene unavailable')
      const scene = game.scene.getScene('mcr-map'), key = `mcr-avatar-${++avatarRevision}`, ownedCanvas = document.createElement('canvas')
      ownedCanvas.width = canvas.width; ownedCanvas.height = canvas.height
      const context = ownedCanvas.getContext('2d'); if (!context) throw new Error('Avatar texture unavailable')
      context.drawImage(canvas, 0, 0) // Phaser owns this copy; texture removal cannot erase the shared thumbnail/cache pixels.
      const texture = scene.textures.addCanvas(key, ownedCanvas)
      if (!texture) throw new Error('Avatar texture unavailable')
      try {
        if (stopped || !sceneReady) throw new Error('Avatar scene unavailable')
        texture.setFilter(Phaser.Textures.FilterMode.NEAREST)
        for (let frame = 0; frame < frames; frame++) texture.add(frame, 0, frame % 5 * 64, Math.floor(frame / 5) * 64, 64, 64)
        let removed = false
        const lease: AvatarSceneTexture = { key, dispose() {
          if (removed) return; removed = true
          for (const [sprite, remove] of Array.from(avatarSprites)) if (spriteTextures.get(sprite) === lease) remove()
          avatarTextures.delete(lease)
          if (scene.textures.exists(key)) scene.textures.remove(key)
        } }
        avatarTextures.set(lease, lease.dispose); avatarTextureFrames.set(lease, frames); return lease
      } catch (error) { scene.textures.remove(key); throw error }
    }
    const avatarScene: AvatarScenePort = {
      texture: canvas => createAvatarTexture(canvas, 20),
      staticTexture64: canvas => createAvatarTexture(canvas, 1),
      sprite(texture) {
        if (stopped || !sceneReady || !avatarTextures.has(texture)) throw new Error('Avatar texture unavailable')
        const image = game.scene.getScene('mcr-map').add.sprite(0, 0, texture.key, 0).setOrigin(.5, 60 / 64).setVisible(false)
        let removed = false
        const lease: AvatarSceneSprite = { update(x, y, frame, depth) {
          if (removed || stopped || !Number.isFinite(x) || !Number.isFinite(y) || !Number.isInteger(frame) || frame < 0 || frame >= (avatarTextureFrames.get(texture) ?? 0)
            || !Number.isFinite(depth) || depth < 1 || depth >= 2) throw new Error('Avatar sprite unavailable')
          image.setPosition(x, y).setFrame(frame).setDepth(depth).setVisible(true)
        }, dispose() { if (removed) return; removed = true; avatarSprites.delete(lease); spriteTextures.delete(lease); image.destroy() } }
        avatarSprites.set(lease, lease.dispose); spriteTextures.set(lease, texture); return lease
      },
      onFrame(listener) {
        if (stopped || !sceneReady) throw new Error('Avatar scene unavailable')
        const scene = game.scene.getScene('mcr-map')
        const callback = (time: number) => { if (!stopped && Number.isFinite(time)) listener(time) }
        const remove = () => { scene.events.off(Phaser.Scenes.Events.UPDATE, callback); avatarListeners.delete(remove) }
        scene.events.on(Phaser.Scenes.Events.UPDATE, callback); avatarListeners.add(remove); return remove
      },
      read: () => ({ textures: avatarTextures.size, sprites: avatarSprites.size, listeners: avatarListeners.size }),
    }
    const muralScene: IncumbentMuralScene = (anchor, key) => {
      if (stopped || !sceneReady) throw new Error('Mural scene unavailable')
      const port = createPhaserMuralScene(game.scene.getScene('mcr-map'), anchor, key)
      const textures = new WeakMap<MuralSceneTexture, MuralSceneTexture>()
      const live = () => { if (stopped || !sceneReady) throw new Error('Mural scene unavailable') }
      return {
        texture() {
          live(); const owned = port.texture()
          const lease: MuralSceneTexture = { replace(rgba) { live(); owned.replace(rgba) }, dispose() { textures.delete(lease); owned.dispose() } }
          textures.set(lease, owned); return lease
        },
        object(texture) { live(); const owned = textures.get(texture); if (!owned) throw new Error('Unowned mural texture'); return port.object(owned) },
        onFrame(listener) { live(); return port.onFrame(() => { if (!stopped && sceneReady) listener() }) },
        onClose(listener) {
          if (stopped || !sceneReady) { listener(); return () => {} }
          let active = true
          const close = () => { if (!active) return; active = false; muralClose.delete(close); removeScene(); listener() }
          const removeScene = port.onClose?.(close) ?? (() => {})
          muralClose.add(close)
          return () => { active = false; muralClose.delete(close); removeScene() }
        },
      }
    }
    const spriteTextures = new Map<AvatarSceneSprite, AvatarSceneTexture>()
    return { ready, closed, get effects() { return effects ?? undefined }, camera: cameraView, avatarScene, muralScene, readScenery,
      replaceSkin(next) {
        if (stopped || !sceneReady) throw new Error('Map scene unavailable')
        assertSameWorld(loaded, next)
        const scene = game.scene.getScene('mcr-map') as MapScene
        const prepared = scene.prepareSkin(next), previous = skin
        prepared.show()
        scene.cameras.main.setBackgroundColor(next.theme.palette.background).setRoundPixels(!next.scenery)
        skin = prepared; previous?.remove()
        if (!visible) game.loop.tick()
      },
      resize(width, height) {
        if (stopped || !sceneReady) return
        game.scale.resize(width, height)
        refreshCamera()
        if (!visible) game.loop.tick()
      },
      setVisible(value) { visible = value; if (!stopped && game.isRunning) { if (value) game.loop.wake(); else game.loop.sleep() } },
      destroy() {
        if (stopped) return
        stopped = true; effects?.dispose(); effects = null; closeMural(); clearAvatars(); game.canvas?.remove()
        flushDestroy() // Boot in progress is fenced by postBoot; its mount has already been detached.
      },
    }
  }
}
