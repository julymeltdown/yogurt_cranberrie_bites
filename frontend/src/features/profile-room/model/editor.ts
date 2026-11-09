/**
 * WP-07 T-609: the room editor reducer.
 *
 * The reducer is pure and synchronous: it owns the *draft* layout, never the
 * saved one. That separation is what makes a revision conflict safe — a conflict
 * can replace the saved room and the draft survives untouched, so the user's
 * unsaved work is never silently discarded.
 *
 * Every placement mutation is reachable by keyboard: the reducer takes a target
 * and a direction, and the UI binds those to buttons and arrow keys. There is no
 * pointer-only path.
 */
import {
  cellFree,
  clampCell,
  GRID_CELLS,
  MAX_FURNITURE,
  MAX_POSTERS,
  sameCell,
  sameLayout,
  slotRect,
  type CharacterPlacement,
  type FloorStyle,
  type FurnitureItem,
  type PosterPlacement,
  type RoomLayout,
  type WallStyle,
} from './layout'

export interface EditorState {
  /** The layout the user is editing. */
  readonly draft: RoomLayout
  /** The layout the server last confirmed, or null before the first save/read. */
  readonly saved: RoomLayout | null
  /** The revision the draft is based on. */
  readonly revision: number
  /** A short message for the live region; never a raw server payload. */
  readonly message: string | null
  /** The item the keyboard/selection cursor is on, if any. */
  readonly selected: Selection | null
  /** A conflict awaiting an explicit resolution. */
  readonly conflict: Conflict | null
  /** Whether a save is in flight. */
  readonly saving: boolean
}

export type Selection =
  | { readonly kind: 'furniture'; readonly index: number }
  | { readonly kind: 'poster'; readonly slot: number }
  | { readonly kind: 'character' }

export interface Conflict {
  /** The current layout the server returned with the 409. */
  readonly current: RoomLayout
  readonly currentRevision: number
}

export type EditorAction =
  | { readonly type: 'loaded'; readonly layout: RoomLayout; readonly revision: number }
  | { readonly type: 'wall'; readonly wall: WallStyle }
  | { readonly type: 'floor'; readonly floor: FloorStyle }
  | { readonly type: 'add-furniture'; readonly item: FurnitureItem }
  | { readonly type: 'remove-furniture'; readonly index: number }
  | { readonly type: 'move-furniture'; readonly index: number; readonly dx: number; readonly dy: number }
  | { readonly type: 'layer-furniture'; readonly index: number; readonly delta: number }
  | { readonly type: 'select'; readonly selection: Selection | null }
  | { readonly type: 'place-poster'; readonly poster: PosterPlacement }
  | { readonly type: 'remove-poster'; readonly slot: number }
  | { readonly type: 'move-poster'; readonly slot: number; readonly dx: number; readonly dy: number }
  | { readonly type: 'move-character'; readonly dx: number; readonly dy: number }
  | { readonly type: 'face'; readonly facing: CharacterMove }
  | { readonly type: 'saving' }
  | { readonly type: 'saved'; readonly layout: RoomLayout; readonly revision: number }
  | { readonly type: 'conflict'; readonly current: RoomLayout; readonly currentRevision: number }
  | { readonly type: 'failed'; readonly message: string }
  | { readonly type: 'keep-draft' }
  | { readonly type: 'use-server' }

type CharacterMove = CharacterPlacement['facing']

/** Whether the draft differs from the last confirmed layout. */
export const isDirty = (state: EditorState) => !state.saved || !sameLayout(state.draft, state.saved)

function withFurniture(layout: RoomLayout, furniture: RoomLayout['furniture']): RoomLayout {
  return { ...layout, furniture }
}

function withPosters(layout: RoomLayout, posters: RoomLayout['posters']): RoomLayout {
  return { ...layout, posters }
}

export function reduce(state: EditorState, action: EditorAction): EditorState {
  switch (action.type) {
    case 'loaded':
      return { ...state, draft: action.layout, saved: action.layout, revision: action.revision, conflict: null, message: null, selected: null }
    case 'wall':
      return { ...state, draft: { ...state.draft, wall: action.wall }, message: null }
    case 'floor':
      return { ...state, draft: { ...state.draft, floor: action.floor }, message: null }
    case 'add-furniture': {
      const { draft } = state
      if (draft.furniture.length >= MAX_FURNITURE) return { ...state, message: `A room holds at most ${MAX_FURNITURE} furniture items.` }
      // The first free cell, scanning from the back of the floor forwards, so a
      // new item never lands on top of an existing one.
      let placed: RoomLayout['furniture'][number] | null = null
      for (let y = GRID_CELLS - 2; y >= 0 && !placed; y--) {
        for (let x = 0; x < GRID_CELLS && !placed; x++) {
          if (cellFree(draft, x, y, 0)) placed = { item_id: action.item, cell_x: x, cell_y: y, layer: 0 }
        }
      }
      if (!placed) return { ...state, message: 'There is no free cell for another piece of furniture.' }
      const furniture = [...draft.furniture, placed]
      return { ...state, draft: withFurniture(draft, furniture), selected: { kind: 'furniture', index: furniture.length - 1 }, message: `Added ${action.item}.` }
    }
    case 'remove-furniture': {
      if (action.index < 0 || action.index >= state.draft.furniture.length) return state
      const furniture = state.draft.furniture.filter((_, index) => index !== action.index)
      const selected = state.selected?.kind === 'furniture' && state.selected.index === action.index ? null : state.selected
      return { ...state, draft: withFurniture(state.draft, furniture), selected, message: 'Removed furniture.' }
    }
    case 'move-furniture': {
      const current = state.draft.furniture[action.index]
      if (!current) return state
      const cellX = clampCell(current.cell_x + action.dx), cellY = clampCell(current.cell_y + action.dy)
      if (sameCell(cellX, current.cell_x) && sameCell(cellY, current.cell_y)) return state
      if (!cellFree(state.draft, cellX, cellY, current.layer, action.index)) {
        return { ...state, message: 'That cell is already occupied in this layer.' }
      }
      const furniture = state.draft.furniture.map((placement, index) => index === action.index ? { ...placement, cell_x: cellX, cell_y: cellY } : placement)
      return { ...state, draft: withFurniture(state.draft, furniture), message: null }
    }
    case 'layer-furniture': {
      const current = state.draft.furniture[action.index]
      if (!current) return state
      const layer = current.layer + action.delta
      if (layer < 0 || layer > 3) return state
      if (!cellFree(state.draft, current.cell_x, current.cell_y, layer, action.index)) {
        return { ...state, message: 'That cell is already occupied in this layer.' }
      }
      const furniture = state.draft.furniture.map((placement, index) => index === action.index ? { ...placement, layer } : placement)
      return { ...state, draft: withFurniture(state.draft, furniture), message: null }
    }
    case 'select':
      return { ...state, selected: action.selection }
    case 'place-poster': {
      const { draft } = state
      const slot = action.poster.slot
      if (slot < 0 || slot >= MAX_POSTERS) return state
      const replaced = draft.posters.filter(poster => poster.slot !== slot)
      if (replaced.length >= MAX_POSTERS) return { ...state, message: `A room holds at most ${MAX_POSTERS} posters.` }
      if (replaced.some(poster => poster.poster_media_id === action.poster.poster_media_id)) {
        return { ...state, message: 'That poster is already on the wall.' }
      }
      const posters = [...replaced, action.poster].sort((a, b) => a.slot - b.slot)
      return { ...state, draft: withPosters(draft, posters), selected: { kind: 'poster', slot }, message: null }
    }
    case 'remove-poster': {
      const posters = state.draft.posters.filter(poster => poster.slot !== action.slot)
      if (posters.length === state.draft.posters.length) return state
      const selected = state.selected?.kind === 'poster' && state.selected.slot === action.slot ? null : state.selected
      return { ...state, draft: withPosters(state.draft, posters), selected, message: 'Removed poster.' }
    }
    case 'move-poster': {
      const current = state.draft.posters.find(poster => poster.slot === action.slot)
      if (!current) return state
      const x = Math.max(0, Math.min(GRID_CELLS - current.width, current.x + action.dx))
      const y = Math.max(0, Math.min(GRID_CELLS - current.height, current.y + action.dy))
      if (x === current.x && y === current.y) return state
      const posters = state.draft.posters.map(poster => poster.slot === action.slot ? { ...poster, x, y } : poster)
      return { ...state, draft: withPosters(state.draft, posters), message: null }
    }
    case 'move-character': {
      const character = state.draft.character
      const cellX = clampCell(character.cell_x + action.dx), cellY = clampCell(character.cell_y + action.dy)
      if (sameCell(cellX, character.cell_x) && sameCell(cellY, character.cell_y)) return state
      return { ...state, draft: { ...state.draft, character: { ...character, cell_x: cellX, cell_y: cellY } }, message: null }
    }
    case 'face':
      return { ...state, draft: { ...state.draft, character: { ...state.draft.character, facing: action.facing } }, message: null }
    case 'saving':
      return { ...state, saving: true, message: 'Saving your room…' }
    case 'saved':
      return { ...state, draft: action.layout, saved: action.layout, revision: action.revision, saving: false, conflict: null, message: 'Room saved.' }
    case 'conflict':
      // The draft is deliberately untouched: the user's unsaved layout survives a
      // conflict and the resolution is explicit.
      return { ...state, saving: false, conflict: { current: action.current, currentRevision: action.currentRevision }, message: 'Someone saved this room while you were editing. Your layout is kept.' }
    case 'failed':
      return { ...state, saving: false, message: action.message }
    case 'keep-draft':
      // Keep the user's layout but rebase it on the server's current revision, so
      // the next save is a deliberate overwrite of the newer room.
      return { ...state, revision: state.conflict?.currentRevision ?? state.revision, saved: state.conflict?.current ?? state.saved, conflict: null, message: 'Your layout is kept and will replace the newer room when you save.' }
    case 'use-server': {
      const current = state.conflict?.current
      if (!current) return state
      return { ...state, draft: current, saved: current, revision: state.conflict?.currentRevision ?? state.revision, conflict: null, message: 'The saved room is now shown.' }
    }
  }
}

/** The default poster rectangle for a slot, used when a poster is first placed. */
export const defaultPosterPlacement = (mediaId: string, slot: number): PosterPlacement => {
  const rect = slotRect(slot)
  return { poster_media_id: mediaId, slot, ...rect }
}
