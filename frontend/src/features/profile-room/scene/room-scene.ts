/**
 * WP-07 T-608: the room scene runtime.
 *
 * The room is a *scoped* Phaser instance: one game per mounted room screen,
 * acquired when the screen mounts and destroyed when it unmounts. Nothing here
 * touches the world runtime's engine lease, so a room screen and the world screen
 * can never fight over one Phaser instance.
 *
 * The rules this module exists to keep:
 *
 * * **Nearest-neighbour everywhere.** The room is pixel art; a smoothing filter
 *   would blur it. Every texture is registered with `FilterMode.NEAREST` and the
 *   game is created with `pixelArt: true, antialias: false, roundPixels: true`.
 * * **Depth by foot position.** Furniture, posters and the character are drawn in
 *   the order a viewer standing in the room would see them: a lower foot position
 *   is nearer and therefore in front. The depth function is the world renderer's
 *   `sceneryDepth`, reused rather than re-invented.
 * * **Failure cleanup.** A texture or object that fails half way through
 *   preparation is released before the error is reported, so a failed load never
 *   leaves a texture behind.
 * * **No presence.** The character is a decoration of the room. There is no
 *   network subscription, no heartbeat, no "online" badge and no session state in
 *   this module; the scene cannot know whether the owner is connected, and
 *   therefore cannot show it.
 *
 * The scene is deliberately not a copy of `MapScene`: a room is a fixed pixel-art
 * space with a handful of objects, not a tilemap world with a camera, controls,
 * avatars and murals. What it reuses is the depth rule and the texture discipline.
 */
import type PhaserType from 'phaser'
import { sceneryDepth } from '../../../world/illustrated/model'
import { GRID_CELLS, type RoomLayout } from '../model/layout'

/** The side of one room cell in scene pixels. The room is a fixed 32×32 grid. */
export const CELL_PX = 16
/** The scene's pixel size: the whole grid at `CELL_PX` per cell. */
export const SCENE_PX = GRID_CELLS * CELL_PX

/** The closed wall palette. Each style is a two-tone pixel pattern, not a photo. */
const WALL_COLORS: Readonly<Record<RoomLayout['wall'], { readonly base: number; readonly stripe: number }>> = Object.freeze({
  plaster: { base: 0xe8dcc8, stripe: 0xded0b8 },
  brick: { base: 0xb5644a, stripe: 0x9c523c },
  wallpaper: { base: 0x3f5d7a, stripe: 0x36506a },
  panel: { base: 0x8a8f98, stripe: 0x7c818a },
})

/** The closed floor palette. */
const FLOOR_COLORS: Readonly<Record<RoomLayout['floor'], { readonly base: number; readonly stripe: number }>> = Object.freeze({
  wood: { base: 0x8a5a3b, stripe: 0x7a4e33 },
  tile: { base: 0xc9c4bb, stripe: 0xb6b1a8 },
  rug: { base: 0x8f3f4f, stripe: 0x7d3644 },
  concrete: { base: 0x9a9a9a, stripe: 0x8c8c8c },
})

/** The furniture palette: one flat colour per item, drawn as a solid block. */
const FURNITURE_COLORS: Readonly<Record<RoomLayout['furniture'][number]['item_id'], number>> = Object.freeze({
  bed: 0x6b7fa8,
  chair: 0xa8763f,
  desk: 0x7a5230,
  lamp: 0xe0c060,
  plant: 0x4f8a4a,
  shelf: 0x6a4a30,
  sofa: 0x8a4a6a,
  table: 0x8a6a44,
})

/** The wall occupies the top band of the grid; the floor is everything below. */
export const WALL_ROWS = 14

/** The depth of a room element whose foot is at `footY` scene pixels. */
export const roomDepth = (footY: number) => sceneryDepth(footY, SCENE_PX)

/**
 * The draw list of a layout: one entry per drawn element, already ordered by
 * depth and carrying the exact rectangle it occupies.
 *
 * This is the pure part of the scene, and the part the tests assert: the same
 * layout always produces the same draw list, so a re-entered room restores the
 * saved layout exactly.
 */
export interface RoomDraw {
  readonly kind: 'wall' | 'floor' | 'furniture' | 'poster' | 'character'
  readonly key: string
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
  readonly depth: number
  /** The poster media id, for a poster; the item id, for furniture. */
  readonly ref: string
}

export function roomDrawList(layout: RoomLayout): readonly RoomDraw[] {
  const draws: RoomDraw[] = [
    { kind: 'wall', key: `wall:${layout.wall}`, x: 0, y: 0, width: SCENE_PX, height: WALL_ROWS * CELL_PX, depth: 0, ref: layout.wall },
    { kind: 'floor', key: `floor:${layout.floor}`, x: 0, y: WALL_ROWS * CELL_PX, width: SCENE_PX, height: (GRID_CELLS - WALL_ROWS) * CELL_PX, depth: 0.1, ref: layout.floor },
  ]
  // Posters hang on the wall, behind everything standing on the floor.
  for (const poster of layout.posters) {
    draws.push({
      kind: 'poster',
      key: `poster:${poster.slot}`,
      x: poster.x * CELL_PX,
      y: poster.y * CELL_PX,
      width: poster.width * CELL_PX,
      height: poster.height * CELL_PX,
      depth: 0.2 + poster.slot * 0.01,
      ref: poster.poster_media_id,
    })
  }
  // Furniture and the character share the floor's depth ordering by foot position.
  for (const [index, placement] of layout.furniture.entries()) {
    const foot = (placement.cell_y + 1) * CELL_PX
    draws.push({
      kind: 'furniture',
      key: `furniture:${index}`,
      x: placement.cell_x * CELL_PX,
      y: placement.cell_y * CELL_PX,
      width: CELL_PX,
      height: CELL_PX,
      depth: roomDepth(foot) + placement.layer * 0.001,
      ref: placement.item_id,
    })
  }
  const characterFoot = (layout.character.cell_y + 1) * CELL_PX
  draws.push({
    kind: 'character',
    key: 'character',
    x: layout.character.cell_x * CELL_PX,
    y: layout.character.cell_y * CELL_PX,
    width: CELL_PX,
    height: CELL_PX * 2,
    depth: roomDepth(characterFoot),
    ref: layout.character.facing,
  })
  return Object.freeze(draws)
}

/** The scene's public surface. It has no presence, session or network method. */
export interface RoomScene {
  /** Replace the drawn layout. The scene owns the objects it creates. */
  render(layout: RoomLayout): void
  /** The number of live textures and objects, for the lifecycle assertions. */
  read(): { readonly textures: number; readonly objects: number; readonly destroyed: boolean }
  /** Destroy the game and release every texture. Idempotent. */
  destroy(): void
  /** Resolves when the game has fully stopped. */
  readonly closed: Promise<void>
}

export interface RoomSceneOptions {
  /** The element the canvas is mounted in. */
  readonly host: HTMLElement
  /** Loads a poster image by media id. A rejected promise is a failed load. */
  readonly loadPoster: (mediaId: string) => Promise<HTMLImageElement>
  /** Called once the scene is ready. */
  readonly onReady?: () => void
  /** Called when the scene could not be prepared; the scene is already released. */
  readonly onError?: (reason: 'engine-failed' | 'load-failed') => void
}

/**
 * Create the scoped room scene.
 *
 * The returned handle is the only owner of the Phaser game. `destroy()` releases
 * everything, including textures created for a layout that is no longer drawn.
 */
export async function createRoomScene(options: RoomSceneOptions): Promise<RoomScene> {
  const Phaser = (await import('phaser')).default
  const textures = new Set<string>()
  let objects: PhaserType.GameObjects.GameObject[] = []
  let destroyed = false, ready = false
  let resolveClosed!: () => void
  const closed = new Promise<void>(resolve => { resolveClosed = resolve })
  let game: PhaserType.Game | null = null
  let scene: PhaserType.Scene | null = null

  const removeTextures = () => {
    for (const key of Array.from(textures)) {
      if (game?.textures.exists(key)) game.textures.remove(key)
      textures.delete(key)
    }
  }
  const removeObjects = () => {
    for (const object of objects) object.destroy()
    objects = []
  }
  /** Release everything this scene acquired. Safe to call more than once. */
  const release = () => {
    if (destroyed) return
    destroyed = true
    removeObjects()
    removeTextures()
    if (game) {
      const owned = game
      game = null
      scene = null
      owned.events.once(Phaser.Core.Events.DESTROY, () => resolveClosed())
      owned.destroy(true, false)
    } else {
      resolveClosed()
    }
  }

  class RoomSceneClass extends Phaser.Scene {
    constructor() { super('mcr-profile-room') }
    create() {
      if (destroyed) return
      scene = this
      ready = true
      options.onReady?.()
    }
  }

  try {
    game = new Phaser.Game({
      type: Phaser.CANVAS,
      parent: options.host,
      width: SCENE_PX,
      height: SCENE_PX,
      scene: [RoomSceneClass],
      banner: false,
      audio: { noAudio: true },
      // Pixel art: no smoothing, no antialiasing, whole-pixel placement.
      pixelArt: true,
      antialias: false,
      roundPixels: true,
      transparent: false,
      input: { keyboard: false, mouse: false, touch: false, gamepad: false },
      scale: { mode: Phaser.Scale.FIT, autoCenter: Phaser.Scale.CENTER_BOTH },
    })
  } catch {
    release()
    options.onError?.('engine-failed')
    throw new Error('Room scene unavailable')
  }

  /** Wait for the scene's `create`, which is where drawing is allowed. */
  await new Promise<void>((resolve, reject) => {
    const started = Date.now()
    const poll = () => {
      if (destroyed) return reject(new Error('Room scene disposed'))
      if (ready) return resolve()
      if (Date.now() - started > 5_000) return reject(new Error('Room scene timeout'))
      setTimeout(poll, 0)
    }
    poll()
  })

  /** One flat rectangle texture, drawn as a solid pixel block. */
  function flatTexture(key: string, color: number, width: number, height: number, stripe?: number) {
    if (!game || !scene) throw new Error('Room scene unavailable')
    if (game.textures.exists(key)) return key
    const canvas = document.createElement('canvas')
    canvas.width = width
    canvas.height = height
    const context = canvas.getContext('2d')
    if (!context) throw new Error('Room scene unavailable')
    context.fillStyle = `#${color.toString(16).padStart(6, '0')}`
    context.fillRect(0, 0, width, height)
    if (stripe !== undefined) {
      // A deterministic two-tone pattern: every other row band is the stripe
      // colour, which reads as brickwork, planks or wallpaper at this scale.
      context.fillStyle = `#${stripe.toString(16).padStart(6, '0')}`
      for (let y = 0; y < height; y += 4) context.fillRect(0, y, width, 2)
    }
    textures.add(key)
    const texture = game.textures.addCanvas(key, canvas)
    if (!texture) throw new Error('Room texture unavailable')
    texture.setFilter(Phaser.Textures.FilterMode.NEAREST)
    return key
  }

  return {
    async render(layout: RoomLayout) {
      if (destroyed || !scene) throw new Error('Room scene unavailable')
      // Build the whole draw list first; a failure releases what was prepared.
      const prepared: { key: string; draw: RoomDraw }[] = []
      try {
        for (const draw of roomDrawList(layout)) {
          if (draw.kind === 'wall') {
            const colors = WALL_COLORS[layout.wall]
            prepared.push({ key: flatTexture(`room-wall-${layout.wall}`, colors.base, SCENE_PX, WALL_ROWS * CELL_PX, colors.stripe), draw })
          } else if (draw.kind === 'floor') {
            const colors = FLOOR_COLORS[layout.floor]
            prepared.push({ key: flatTexture(`room-floor-${layout.floor}`, colors.base, SCENE_PX, (GRID_CELLS - WALL_ROWS) * CELL_PX, colors.stripe), draw })
          } else if (draw.kind === 'furniture') {
            const color = FURNITURE_COLORS[layout.furniture[Number(draw.key.split(':')[1])]!.item_id]
            prepared.push({ key: flatTexture(`room-furniture-${draw.ref}`, color, CELL_PX, CELL_PX), draw })
          } else if (draw.kind === 'character') {
            prepared.push({ key: flatTexture('room-character', 0x22252c, CELL_PX, CELL_PX * 2), draw })
          } else {
            const image = await options.loadPoster(draw.ref)
            if (destroyed || !scene) throw new Error('Room scene disposed')
            const key = `room-poster-${draw.ref}`
            if (!game!.textures.exists(key)) {
              textures.add(key)
              const texture = game!.textures.addImage(key, image)
              if (!texture) throw new Error('Room poster texture unavailable')
              texture.setFilter(Phaser.Textures.FilterMode.NEAREST)
            }
            prepared.push({ key, draw })
          }
        }
      } catch (error) {
        // A failed load releases everything acquired, including the objects of the
        // previous layout, so a half-drawn room is never shown.
        removeObjects()
        removeTextures()
        options.onError?.('load-failed')
        throw error instanceof Error ? error : new Error('Room load failed')
      }
      if (destroyed || !scene) throw new Error('Room scene disposed')
      const next: PhaserType.GameObjects.GameObject[] = []
      try {
        for (const { key, draw } of prepared) {
          const image = scene.add.image(draw.x, draw.y, key).setOrigin(0).setDepth(draw.depth)
          if (draw.kind === 'furniture' || draw.kind === 'character') {
            image.setDisplaySize(draw.width, draw.height)
          }
          next.push(image)
        }
      } catch (error) {
        for (const object of next) object.destroy()
        removeObjects()
        removeTextures()
        options.onError?.('load-failed')
        throw error
      }
      // Only now is the previous layout replaced: the new one is complete.
      removeObjects()
      objects = next
    },
    read: () => ({
      textures: game ? game.textures.getTextureKeys().filter(key => key.startsWith('room-')).length : 0,
      objects: objects.length,
      destroyed,
    }),
    destroy: release,
    closed,
  }
}
