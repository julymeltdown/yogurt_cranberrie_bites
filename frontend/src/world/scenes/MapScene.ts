import type PhaserType from 'phaser'
import type { EngineFactory } from '../runtime'
/** Phaser is evaluated only after an explicitly started owner has loaded and verified resources. */
export async function loadPhaserEngine(): Promise<EngineFactory> {
  const Phaser = (await import('phaser')).default
  return (host, loaded) => {
    let stopped = false, destroyed = false, visible = true
    let resolveReady!: () => void, rejectReady!: (reason: unknown) => void, resolveClosed!: () => void
    const ready = new Promise<void>((resolve, reject) => { resolveReady = resolve; rejectReady = reject })
    const closed = new Promise<void>(resolve => { resolveClosed = resolve })
    const textureKey = 'mcr-reviewed-map', g = loaded.map.geometry
    let sceneReady = false
    class MapScene extends Phaser.Scene {
      constructor() { super('mcr-map') }
      create() {
        try {
          if (stopped) return
          sceneReady = true
          const texture = this.textures.addImage(textureKey, loaded.image)
          if (!texture) throw new Error('Map texture unavailable')
          texture.setFilter(Phaser.Textures.FilterMode.NEAREST)
          const tilemap = this.make.tilemap({ tileWidth: g.tile_width_px, tileHeight: g.tile_height_px, width: g.width_cells, height: g.height_cells })
          const tiles = tilemap.addTilesetImage('theme', textureKey, g.tile_width_px, g.tile_height_px, 0, 0, 0)
          if (!tiles) throw new Error('Map tiles unavailable')
          for (const [name, grid, depth] of [['terrain', g.terrain_layer, 0], ['overdraw', g.overdraw_layer, 2]] as const) {
            const layer = tilemap.createBlankLayer(name, tiles)
            if (!layer) throw new Error('Map layer unavailable')
            layer.putTilesAt(grid.map(row => row.map(id => id === 0 ? -1 : id - 1)), 0, 0).setDepth(depth)
          }
          let removed = false
          const remove = () => { if (!removed) { removed = true; tilemap.destroy() } }
          this.events.once(Phaser.Scenes.Events.SHUTDOWN, remove)
          this.events.once(Phaser.Scenes.Events.DESTROY, remove)
          this.cameras.main.setBounds(0, 0, loaded.widthPx, loaded.heightPx).setRoundPixels(true)
          this.cameras.main.setBackgroundColor(loaded.theme.palette.background)
          resolveReady()
        } catch (error) { rejectReady(error) }
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
    return { ready, closed,
      resize(width, height) {
        if (stopped || !sceneReady) return
        const scene = game.scene.getScene('mcr-map')
        game.scale.resize(width, height)
        const camera = scene.cameras.main
        camera.setSize(width, height).setZoom(Math.max(1, Math.floor(Math.min(width / loaded.widthPx, height / loaded.heightPx))))
        camera.centerOn((loaded.spawn.cell_x + .5) * g.tile_width_px, (loaded.spawn.cell_y + .5) * g.tile_height_px)
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
