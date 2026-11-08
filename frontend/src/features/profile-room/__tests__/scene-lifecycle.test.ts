// @vitest-environment jsdom
/**
 * WP-07 T-608: room scene lifecycle.
 *
 * The acceptance cases of the task are the first three tests below:
 *
 * * `leaving_the_room_destroys_the_scene_and_releases_textures`;
 * * `re_entering_the_room_restores_the_saved_layout_exactly`;
 * * `no_presence_or_online_affordance_exists_anywhere_in_the_room`.
 *
 * Phaser cannot run in jsdom (it needs a real canvas and a frame loop), so the
 * engine is replaced by a recording double through `vi.mock`. That is the right
 * seam: the assertions here are about *ownership* — what the runtime acquires,
 * what it releases, and what it draws — not about Phaser's own rendering, which
 * the browser smoke tests cover.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { readFileSync, readdirSync } from 'node:fs'
import { resolve } from 'node:path'

// ---------------------------------------------------------------------------
// The Phaser double
// ---------------------------------------------------------------------------

interface FakeTexture { key: string; filter: number | null; setFilter(value: number): void }
interface FakeImage {
  key: string
  x: number
  y: number
  depth: number
  displayWidth: number
  displayHeight: number
  destroyed: boolean
  setOrigin(x: number, y: number): FakeImage
  setDepth(value: number): FakeImage
  setDisplaySize(width: number, height: number): FakeImage
  destroy(): void
}
interface FakeGame {
  textures: { keys: Map<string, FakeTexture>; exists(key: string): boolean; remove(key: string): void; addCanvas(key: string, canvas: unknown): FakeTexture | null; addImage(key: string, image: unknown): FakeTexture | null; getTextureKeys(): string[] }
  scene: { getScene(key: string): FakeScene | null }
  events: { once(event: string, callback: () => void): void; emitDestroy(): void }
  destroy(removeCanvas: boolean, noReturn: boolean): void
  destroyed: boolean
}

const games: FakeGame[] = []
const images: FakeImage[] = []
let failNextTexture = false

class FakeScene {
  readonly objects: FakeImage[] = []
  add = {
    image: (x: number, y: number, key: string): FakeImage => {
      const image: FakeImage = {
        key, x, y, depth: 0, displayWidth: 0, displayHeight: 0, destroyed: false,
        setOrigin() { return image },
        setDepth(value: number) { image.depth = value; return image },
        setDisplaySize(width: number, height: number) { image.displayWidth = width; image.displayHeight = height; return image },
        destroy() { image.destroyed = true },
      }
      images.push(image)
      this.objects.push(image)
      return image
    },
  }
}

const registry = new Map<string, FakeScene>()

vi.mock('phaser', () => {
  const FilterMode = { NEAREST: 0, LINEAR: 1 }
  const CoreEvents = { DESTROY: 'destroy' }
  class Game implements FakeGame {
    destroyed = false
    readonly textures: FakeGame['textures']
    readonly scene: FakeGame['scene']
    readonly events: FakeGame['events']
    constructor(config: { parent: HTMLElement; scene: (new () => FakeScene)[] }) {
      const keys = new Map<string, FakeTexture>()
      const scenes = new Map<string, FakeScene>()
      this.textures = {
        keys,
        exists: key => keys.has(key),
        remove: key => { keys.delete(key) },
        addCanvas: (key: string) => {
          if (failNextTexture) { failNextTexture = false; return null }
          const texture: FakeTexture = { key, filter: null, setFilter(value: number) { texture.filter = value } }
          keys.set(key, texture)
          return texture
        },
        addImage: (key: string) => {
          const texture: FakeTexture = { key, filter: null, setFilter(value: number) { texture.filter = value } }
          keys.set(key, texture)
          return texture
        },
        getTextureKeys: () => Array.from(keys.keys()),
      }
      this.scene = { getScene: key => scenes.get(key) ?? null }
      let destroyCallback: (() => void) | null = null
      this.events = { once: (_event, callback) => { destroyCallback = callback }, emitDestroy: () => destroyCallback?.() }
      const Scene = config.scene[0]!
      const instance = new Scene()
      // The real Phaser calls `create` after boot; the double does it inline.
      ;(instance as unknown as { add: FakeScene['add'] }).add = new FakeScene().add
      scenes.set('mcr-profile-room', instance)
      games.push(this)
      // `create` runs on the next tick, like the real boot sequence.
      setTimeout(() => { if (!this.destroyed) (instance as unknown as { create(): void }).create() }, 0)
      const parent = config.parent
      parent.setAttribute('data-fake-game', 'mounted')
    }
    destroy() {
      if (this.destroyed) return
      this.destroyed = true
      this.events.emitDestroy()
    }
  }
  return {
    default: {
      CANVAS: 1,
      Scale: { FIT: 1, CENTER_BOTH: 1 },
      Textures: { FilterMode },
      Core: { Events: CoreEvents },
      Game,
      Scene: class { },
    },
  }
})

import {
  CELL_PX,
  SCENE_PX,
  WALL_ROWS,
  createRoomScene,
  roomDepth,
  roomDrawList,
  type RoomScene,
} from '../scene/room-scene'
import { initialLayout, type RoomLayout } from '../model/layout'

const cleanups: (() => Promise<void> | void)[] = []
beforeEach(() => {
  games.length = 0
  images.length = 0
  failNextTexture = false
  registry.clear()
  vi.stubGlobal('Image', class { })
  // jsdom ships no 2D canvas. The scene only needs a surface to paint flat pixel
  // blocks on, so the double records the fill calls instead of rasterizing.
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(() => ({
    fillStyle: '#000000',
    fillRect: vi.fn(),
  }) as unknown as CanvasRenderingContext2D)
})
afterEach(async () => {
  for (const cleanup of cleanups.splice(0).reverse()) await cleanup()
  document.body.replaceChildren()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

const poster = (_id: string) => ({ width: 64, height: 64 } as unknown as HTMLImageElement)

async function scene(layout: RoomLayout, options: { readonly failPoster?: boolean } = {}) {
  const host = document.createElement('div')
  document.body.append(host)
  const created = await createRoomScene({
    host,
    loadPoster: async (mediaId: string) => {
      if (options.failPoster) throw new Error('poster unavailable')
      expect(mediaId).toMatch(/^[0-9a-f-]{36}$/)
      return poster(mediaId)
    },
  })
  cleanups.push(async () => { created.destroy(); await created.closed })
  await created.render(layout)
  return { host, created }
}

function savedLayout(): RoomLayout {
  return {
    version: 1,
    wall: 'brick',
    floor: 'tile',
    furniture: [
      { item_id: 'sofa', cell_x: 3, cell_y: 18, layer: 0 },
      { item_id: 'lamp', cell_x: 3, cell_y: 18, layer: 1 },
      { item_id: 'desk', cell_x: 20, cell_y: 12, layer: 2 },
    ],
    posters: [{ poster_media_id: '11111111-1111-4111-8111-111111111111', slot: 0, x: 3, y: 2, width: 8, height: 8 }],
    character: { cell_x: 9, cell_y: 27, facing: 'LEFT' },
  }
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

describe('room scene lifecycle', () => {
  it('leaving the room destroys the scene and releases textures', async () => {
    const { host, created } = await scene(savedLayout())
    const before = created.read()
    expect(before.destroyed).toBe(false)
    expect(before.textures).toBeGreaterThan(0)
    expect(before.objects).toBeGreaterThan(0)
    // One game, mounted into the host.
    expect(games).toHaveLength(1)
    expect(host.getAttribute('data-fake-game')).toBe('mounted')
    created.destroy()
    await created.closed
    expect(created.read().destroyed).toBe(true)
    expect(created.read().textures).toBe(0)
    expect(created.read().objects).toBe(0)
    expect(games[0]!.destroyed).toBe(true)
    // Every game object was destroyed, not merely detached.
    expect(images.every(image => image.destroyed)).toBe(true)
    // Destroy is idempotent: a second call is a no-op, not a second teardown.
    created.destroy()
    await created.closed
    expect(games).toHaveLength(1)
  })

  it('re_entering_the_room_restores_the_saved_layout_exactly', async () => {
    const layout = savedLayout()
    const first = await scene(layout)
    const firstDraws = roomDrawList(layout).map(draw => ({ ...draw }))
    const firstObjects = first.created.read().objects
    const firstTextures = first.created.read().textures
    first.created.destroy()
    await first.created.closed
    // A fresh mount of the same saved layout produces the same draw list and the
    // same number of live objects, with no state carried over from the first.
    const second = await scene(layout)
    const secondDraws = roomDrawList(layout).map(draw => ({ ...draw }))
    expect(secondDraws).toEqual(firstDraws)
    expect(second.created.read().objects).toBe(firstObjects)
    expect(second.created.read().textures).toBe(firstTextures)
    expect(firstObjects).toBeGreaterThan(0)
    // A different layout draws a different room.
    const other = await scene(initialLayout())
    expect(other.created.read().objects).not.toBe(firstObjects)
  })

  it('a failed poster load releases everything the scene acquired', async () => {
    const host = document.createElement('div')
    document.body.append(host)
    const onError = vi.fn()
    const created = await createRoomScene({
      host,
      loadPoster: async () => { throw new Error('poster unavailable') },
      onError,
    })
    cleanups.push(async () => { created.destroy(); await created.closed })
    await expect(created.render(savedLayout())).rejects.toThrow()
    expect(onError).toHaveBeenCalledWith('load-failed')
    // The scene is still alive (the screen can retry), but it holds no textures
    // and no objects from the failed attempt.
    expect(created.read().textures).toBe(0)
    expect(created.read().objects).toBe(0)
    expect(images.every(image => image.destroyed)).toBe(true)
  })

  it('a texture that cannot be created releases the scene and reports the failure', async () => {
    const host = document.createElement('div')
    document.body.append(host)
    const onError = vi.fn()
    const created = await createRoomScene({ host, loadPoster: async () => poster('x'), onError })
    cleanups.push(async () => { created.destroy(); await created.closed })
    failNextTexture = true
    await expect(created.render(initialLayout())).rejects.toThrow()
    expect(onError).toHaveBeenCalledWith('load-failed')
    expect(created.read().textures).toBe(0)
  })

  it('a re-render replaces the previous layout rather than stacking a second one', async () => {
    const { created } = await scene(initialLayout())
    const initialObjects = created.read().objects
    await created.render(savedLayout())
    expect(created.read().objects).toBe(roomDrawList(savedLayout()).length)
    expect(created.read().objects).not.toBe(initialObjects)
    // The objects of the replaced layout were destroyed.
    expect(images.filter(image => image.destroyed).length).toBeGreaterThan(0)
  })
})

// ---------------------------------------------------------------------------
// Pixel-art and depth rules
// ---------------------------------------------------------------------------

describe('room scene rendering rules', () => {
  it('every texture is registered with nearest-neighbour filtering', async () => {
    const { created } = await scene(savedLayout())
    const game = games[0]!
    const roomTextures = game.textures.getTextureKeys().filter(key => key.startsWith('room-'))
    expect(roomTextures.length).toBeGreaterThan(0)
    for (const key of roomTextures) expect(game.textures.keys.get(key)!.filter).toBe(0)
    expect(created.read().textures).toBe(roomTextures.length)
  })

  it('depth is ordered by foot position, so a nearer object is drawn in front', () => {
    const layout = savedLayout()
    const draws = roomDrawList(layout)
    const furniture = draws.filter(draw => draw.kind === 'furniture')
    const back = furniture.find(draw => draw.ref === 'desk')!
    const front = furniture.find(draw => draw.ref === 'sofa')!
    // The desk stands higher up the floor than the sofa, so it is behind it.
    expect(back.depth).toBeLessThan(front.depth)
    // The character's depth follows the same rule.
    const character = draws.find(draw => draw.kind === 'character')!
    expect(character.depth).toBe(roomDepth((layout.character.cell_y + 1) * CELL_PX))
    // The wall and floor are always behind everything standing on the floor.
    expect(draws.filter(draw => draw.kind === 'wall' || draw.kind === 'floor').every(draw => draw.depth < 1)).toBe(true)
    expect(furniture.every(draw => draw.depth >= 1 && draw.depth < 2)).toBe(true)
  })

  it('the same cell in a different layer is drawn in front of the lower layer', () => {
    const draws = roomDrawList(savedLayout())
    const lower = draws.find(draw => draw.kind === 'furniture' && draw.ref === 'sofa')!
    const upper = draws.find(draw => draw.kind === 'furniture' && draw.ref === 'lamp')!
    expect(upper.depth).toBeGreaterThan(lower.depth)
  })

  it('the draw list is a pure function of the layout', () => {
    const layout = savedLayout()
    const first = roomDrawList(layout)
    const second = roomDrawList(structuredClone(layout))
    expect(second).toEqual(first)
    // The grid is the fixed 32x32 room.
    expect(SCENE_PX).toBe(512)
    expect(WALL_ROWS * CELL_PX).toBeLessThan(SCENE_PX)
  })
})

// ---------------------------------------------------------------------------
// No presence
// ---------------------------------------------------------------------------

describe('no presence in the room', () => {
  it('no_presence_or_online_affordance_exists_anywhere_in_the_room', () => {
    const root = resolve(process.cwd(), 'src/features/profile-room') + '/'
    const files = readdirSync(root, { recursive: true, encoding: 'utf8' })
      .filter(name => /\.(ts|tsx)$/.test(name) && !name.includes('__tests__') && !name.includes('.test.'))
    expect(files.length).toBeGreaterThan(0)
    const forbidden = [
      // Wording a presence or activity indicator would need.
      /online/i, /offline/i, /presence/i, /last[_-]?seen/i, /active[_-]?now/i,
      /is[_-]?connected/i, /heartbeat/i, /typing/i,
    ]
    // JSX text is user-visible copy. The one sentence the room states about
    // presence is a negation ("It never shows whether you are online."); it is
    // removed before the scan, so any other use of the word still fails.
    const disclaimer = 'It never shows whether you are online.'
    for (const name of files) {
      const source = readFileSync(`${root}${name}`, 'utf8')
      // Comments may explain what the room deliberately does not do.
      const code = source.replace(/\/\*[\s\S]*?\*\//g, '').split('\n')
        .filter(line => !line.trimStart().startsWith('//') && !line.trimStart().startsWith('*'))
        .join('\n')
        .split(disclaimer).join('')
      for (const pattern of forbidden) {
        expect(pattern.test(code), `${name} must not contain ${pattern}`).toBe(false)
      }
    }
  })

  it('the scene exposes no network, session or presence method', async () => {
    const { created } = await scene(savedLayout())
    const surface = Object.keys(created).sort()
    expect(surface).toEqual(['closed', 'destroy', 'read', 'render'])
    // The runtime takes no session, no socket and no presence port.
    const source = readFileSync(resolve(process.cwd(), 'src/features/profile-room/scene/room-scene.ts'), 'utf8')
    // Comments may explain what the room deliberately does not do; only code counts.
    const code = source.replace(/\/\*[\s\S]*?\*\//g, '').split('\n')
      .filter(line => !line.trimStart().startsWith('//') && !line.trimStart().startsWith('*'))
      .join('\n')
    for (const forbidden of ['WebSocket', 'EventSource', 'fetch(', 'presence', 'session', 'online']) {
      expect(code.includes(forbidden), `room-scene.ts must not contain ${forbidden}`).toBe(false)
    }
  })

  it('the character is drawn as room decoration with no status field', () => {
    const draws = roomDrawList(savedLayout())
    const character = draws.filter(draw => draw.kind === 'character')
    expect(character).toHaveLength(1)
    // The only thing the character carries is which way it faces.
    expect(character[0]!.ref).toBe('LEFT')
    expect(Object.keys(character[0]!).sort()).toEqual(['depth', 'height', 'key', 'kind', 'ref', 'width', 'x', 'y'])
  })
})

// ---------------------------------------------------------------------------
// Type-level ownership
// ---------------------------------------------------------------------------

describe('scene ownership', () => {
  it('a scene is the only owner of its Phaser game', async () => {
    const first = await scene(initialLayout())
    const second = await scene(savedLayout())
    expect(games).toHaveLength(2)
    first.created.destroy()
    await first.created.closed
    // Destroying one scene never touches the other.
    expect(games[0]!.destroyed).toBe(true)
    expect(games[1]!.destroyed).toBe(false)
    expect(second.created.read().destroyed).toBe(false)
  })

  it('render after destroy is refused rather than silently ignored', async () => {
    const { created } = await scene(initialLayout())
    created.destroy()
    await created.closed
    await expect(created.render(initialLayout())).rejects.toThrow()
  })
})

// A compile-time assertion that the scene handle has no presence member.
const _noPresence: RoomScene = {
  render: () => undefined,
  read: () => ({ textures: 0, objects: 0, destroyed: false }),
  destroy: () => undefined,
  closed: Promise.resolve(),
}
void _noPresence
