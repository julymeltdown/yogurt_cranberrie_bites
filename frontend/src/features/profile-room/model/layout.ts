/**
 * WP-07 T-609: the client-side room layout value.
 *
 * This is the browser mirror of `social.RoomLayout` (domain-contract.md §5). It is
 * a *form* model, never an authority: the server validates the same rules and its
 * answer is the only one that counts. The mirror exists so the editor can refuse
 * an obviously invalid placement before a round trip, and so the preview and the
 * saved layout cannot disagree about what a cell means.
 *
 * The grid, the poster count and the furniture count are the frozen contract's
 * values, not product choices.
 */
import { z } from 'zod'

/** The fixed grid: 32 cells on each axis, indexed from zero. */
export const GRID_CELLS = 32
/** At most four posters stand in a room. */
export const MAX_POSTERS = 4
/** At most 32 furniture items stand in a room. */
export const MAX_FURNITURE = 32
/** A furniture layer is one of four draw bands. */
export const MAX_LAYER = 3

/** The closed wall styles, in the order the editor offers them. */
export const WALL_STYLES = ['plaster', 'brick', 'wallpaper', 'panel'] as const
/** The closed floor styles. */
export const FLOOR_STYLES = ['wood', 'tile', 'rug', 'concrete'] as const
/** The closed default furniture set. */
export const FURNITURE_ITEMS = ['bed', 'chair', 'desk', 'lamp', 'plant', 'shelf', 'sofa', 'table'] as const

export type WallStyle = typeof WALL_STYLES[number]
export type FloorStyle = typeof FLOOR_STYLES[number]
export type FurnitureItem = typeof FURNITURE_ITEMS[number]
export type Facing = 'LEFT' | 'RIGHT'

const cell = z.number().int().min(0).max(GRID_CELLS - 1)
const styleId = z.enum(WALL_STYLES)
const floorId = z.enum(FLOOR_STYLES)
const itemId = z.enum(FURNITURE_ITEMS)
const uuid = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)

export const furnitureSchema = z.strictObject({
  item_id: itemId,
  cell_x: cell,
  cell_y: cell,
  layer: z.number().int().min(0).max(MAX_LAYER),
})
export type FurniturePlacement = z.infer<typeof furnitureSchema>

export const posterSchema = z.strictObject({
  poster_media_id: uuid,
  slot: z.number().int().min(0).max(MAX_POSTERS - 1),
  x: z.number().int().min(0).max(GRID_CELLS - 1),
  y: z.number().int().min(0).max(GRID_CELLS - 1),
  width: z.number().int().min(1).max(GRID_CELLS),
  height: z.number().int().min(1).max(GRID_CELLS),
})
export type PosterPlacement = z.infer<typeof posterSchema>

export const characterSchema = z.strictObject({
  cell_x: cell,
  cell_y: cell,
  facing: z.enum(['LEFT', 'RIGHT']),
})
export type CharacterPlacement = z.infer<typeof characterSchema>

export const layoutSchema = z.strictObject({
  version: z.literal(1),
  wall: styleId,
  floor: floorId,
  furniture: z.array(furnitureSchema).max(MAX_FURNITURE),
  posters: z.array(posterSchema).max(MAX_POSTERS),
  character: characterSchema,
})
export type RoomLayout = z.infer<typeof layoutSchema>

/** The layout a profile that has never saved starts from; mirrors the server. */
export const initialLayout = (): RoomLayout => ({
  version: 1,
  wall: 'plaster',
  floor: 'wood',
  furniture: [
    { item_id: 'plant', cell_x: 2, cell_y: 20, layer: 0 },
    { item_id: 'chair', cell_x: 12, cell_y: 22, layer: 0 },
  ],
  posters: [],
  character: { cell_x: 16, cell_y: 24, facing: 'RIGHT' },
})

/** The structural rules the server also enforces, applied to a local layout. */
export function validateLayout(layout: RoomLayout): string | null {
  const parsed = layoutSchema.safeParse(layout)
  if (!parsed.success) return 'This room layout is not valid.'
  const cells = new Set<string>()
  for (const placement of layout.furniture) {
    const key = `${placement.layer}:${placement.cell_x}:${placement.cell_y}`
    if (cells.has(key)) return 'Two pieces of furniture cannot share a cell in the same layer.'
    cells.add(key)
  }
  const slots = new Set<number>(), media = new Set<string>()
  for (const poster of layout.posters) {
    if (poster.x + poster.width > GRID_CELLS || poster.y + poster.height > GRID_CELLS) return 'A poster must stay inside the wall.'
    if (slots.has(poster.slot)) return 'Two posters cannot share a wall slot.'
    if (media.has(poster.poster_media_id)) return 'The same poster cannot hang twice.'
    slots.add(poster.slot)
    media.add(poster.poster_media_id)
  }
  return null
}

/** Decode a `social.RoomLayout` from the server. Throws on anything else. */
export function decodeLayout(value: unknown): RoomLayout {
  return layoutSchema.parse(value)
}

/** A stable key for a layout, used to tell "unsaved changes" from "no changes". */
export function layoutKey(layout: RoomLayout): string {
  return JSON.stringify({
    wall: layout.wall,
    floor: layout.floor,
    furniture: [...layout.furniture].sort((a, b) => a.layer - b.layer || a.cell_y - b.cell_y || a.cell_x - b.cell_x || a.item_id.localeCompare(b.item_id)),
    posters: [...layout.posters].sort((a, b) => a.slot - b.slot),
    character: layout.character,
  })
}

export const sameLayout = (a: RoomLayout, b: RoomLayout) => layoutKey(a) === layoutKey(b)

/**
 * The cell a clamp keeps inside the grid.
 *
 * A cell coordinate is an integer index: a non-finite or fractional input is
 * floored first, so `-0.5` is cell 0 rather than `-0` (which `Object.is` treats
 * as distinct from `0` and which a `===` comparison against the current cell
 * would miss).
 */
export const clampCell = (value: number) => {
  const integer = Number.isFinite(value) ? Math.trunc(value) : 0
  // `+ 0` normalizes `-0` to `0`: `Object.is(-0, 0)` is false, so an unnormalized
  // clamp at the left edge would look like a move and then be dropped as a no-op.
  return Math.max(0, Math.min(GRID_CELLS - 1, integer)) + 0
}

/**
 * Whether two cell coordinates are the same cell.
 *
 * Compares the *normalized* integers rather than the raw numbers, so a clamped
 * `-0` at the left edge is the same cell as `0` instead of a phantom move.
 */
export const sameCell = (a: number, b: number) => Math.trunc(a) + 0 === Math.trunc(b) + 0

/** Whether a furniture item may occupy a cell in a layer. */
export function cellFree(layout: RoomLayout, cellX: number, cellY: number, layer: number, except?: number): boolean {
  return !layout.furniture.some((placement, index) => index !== except
    && placement.layer === layer && placement.cell_x === cellX && placement.cell_y === cellY)
}

/** The next free poster slot, or null when all four are taken. */
export function nextPosterSlot(layout: RoomLayout): number | null {
  for (let slot = 0; slot < MAX_POSTERS; slot++) {
    if (!layout.posters.some(poster => poster.slot === slot)) return slot
  }
  return null
}

/** The default wall rectangle of a poster slot, in grid cells. */
export function slotRect(slot: number): { x: number; y: number; width: number; height: number } {
  const column = slot % 2, row = Math.floor(slot / 2)
  return { x: 3 + column * 12, y: 2 + row * 10, width: 8, height: 8 }
}
