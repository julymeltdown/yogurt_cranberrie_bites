import type { Point, WorldCamera } from '../camera/port'
import type PhaserType from 'phaser'
import type { EngineFactory } from '../runtime'
import type { LoadedMap } from '../map/resources'
import { assertSameWorld } from '../theme/compatibility'
/** Phaser is evaluated only after an explicitly started owner has loaded and verified resources. */
export async function loadPhaserEngine(): Promise<EngineFactory> {
  const Phaser = (await import('phaser')).default
  return (host, loaded) => {
    let stopped = false, destroyed = false, visible = true
    let resolveReady!: () => void, rejectReady!: (reason: unknown) => void, resolveClosed!: () => void
    const ready = new Promise<void>((resolve, reject) => { resolveReady = resolve; rejectReady = reject })
    const closed = new Promise<void>(resolve => { resolveClosed = resolve })
    const g = loaded.map.geometry
    let skinRevision = 0
    type Skin = { show(): void; remove(): void }
    let skin: Skin | null = null
    let sceneReady = false, cameraFoot: Point | null = null, cameraZoom: number | null = null
    class MapScene extends Phaser.Scene {
      constructor() { super('mcr-map') }
      create() {
        try {
          if (stopped) return
          sceneReady = true
          skin = this.prepareSkin(loaded)
          skin.show()
          const remove = () => { skin?.remove(); skin = null }
          this.events.once(Phaser.Scenes.Events.SHUTDOWN, remove)
          this.events.once(Phaser.Scenes.Events.DESTROY, remove)
          this.cameras.main.setBounds(0, 0, loaded.widthPx, loaded.heightPx).setRoundPixels(true)
          this.cameras.main.setBackgroundColor(loaded.theme.palette.background)
          resolveReady()
        } catch (error) { rejectReady(error) }
      }
      /** Prepare hidden layers before touching the active renderer. No camera/input/network mutation. */
      prepareSkin(next: LoadedMap): Skin {
        const textureKey = `mcr-reviewed-map-${++skinRevision}`
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
      const foot = cameraFoot ?? { x: (loaded.spawn.cell_x + .5) * g.tile_width_px, y: (loaded.spawn.cell_y + .5) * g.tile_height_px }
      camera.centerOn(foot.x, foot.y)
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
    return { ready, closed, camera: cameraView,
      replaceSkin(next) {
        if (stopped || !sceneReady) throw new Error('Map scene unavailable')
        assertSameWorld(loaded, next)
        const scene = game.scene.getScene('mcr-map') as MapScene
        const prepared = scene.prepareSkin(next), previous = skin
        prepared.show()
        scene.cameras.main.setBackgroundColor(next.theme.palette.background)
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
        stopped = true; game.canvas?.remove()
        flushDestroy() // Boot in progress is fenced by postBoot; its mount has already been detached.
      },
    }
  }
}
