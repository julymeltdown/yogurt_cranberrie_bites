/**
 * WP-07 T-609: the room editor.
 *
 * The editor owns the draft layout and the saved layout separately, so a revision
 * conflict can never silently discard the user's work: the draft survives, the
 * conflict is shown, and the resolution is an explicit choice between keeping the
 * draft and using the saved room.
 *
 * Every placement has a keyboard path:
 *
 * * furniture: add with a button, then move with the arrow keys or the on-screen
 *   arrow buttons, change layer with the layer buttons, remove with a button;
 * * posters: place from a slot button, move with the arrow keys or buttons,
 *   remove with a button;
 * * the character: move and turn with buttons;
 * * wall and floor: ordinary radio groups.
 *
 * Nothing here decides visibility or authorization: the screen renders what the
 * server returned and the save is the server's answer.
 */
import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react'
import {
  FLOOR_STYLES,
  FURNITURE_ITEMS,
  GRID_CELLS,
  MAX_FURNITURE,
  MAX_POSTERS,
  WALL_STYLES,
  type FloorStyle,
  type FurnitureItem,
  type RoomLayout,
  type WallStyle,
} from './model/layout'
import { defaultPosterPlacement, isDirty, reduce, type EditorAction, type EditorState } from './model/editor'
import type { PosterPreset } from './pixelize'
import type { RoomClient, RoomVisibility } from './api/room'
import type { PosterConverter } from './worker/client'
import { PosterImporter } from './PosterImporter'
import './profile-room.css'

/** The label of a furniture item, for the buttons and the list. */
const ITEM_LABELS: Readonly<Record<FurnitureItem, string>> = Object.freeze({
  bed: 'Bed', chair: 'Chair', desk: 'Desk', lamp: 'Lamp',
  plant: 'Plant', shelf: 'Shelf', sofa: 'Sofa', table: 'Table',
})
const WALL_LABELS: Readonly<Record<WallStyle, string>> = Object.freeze({
  plaster: 'Plaster', brick: 'Brick', wallpaper: 'Wallpaper', panel: 'Panel',
})
const FLOOR_LABELS: Readonly<Record<FloorStyle, string>> = Object.freeze({
  wood: 'Wood', tile: 'Tile', rug: 'Rug', concrete: 'Concrete',
})

export interface RoomEditorProps {
  readonly client: RoomClient
  readonly converter: PosterConverter
  readonly state: EditorState
  readonly dispatch: (action: EditorAction) => void
  /** The room's current visibility, as the server reported it. */
  readonly visibility: RoomVisibility
  /** The revision the visibility toggle must send. */
  readonly revision: number
  readonly onVisibility: (visibility: RoomVisibility) => void
}

export function RoomEditor({ client, converter, state, dispatch, visibility, revision, onVisibility }: RoomEditorProps) {
  const id = useId()
  const [importing, setImporting] = useState(false)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState<string | null>(null)
  const list = useRef<HTMLOListElement | null>(null)
  const { draft } = state
  const dirty = isDirty(state)

  async function save() {
    setBusy(true)
    dispatch({ type: 'saving' })
    try {
      const saved = await client.save(state.revision, draft)
      dispatch({ type: 'saved', layout: saved.layout, revision: saved.revision })
    } catch (error) {
      const conflict = error as { name?: string; current?: { layout: RoomLayout; revision: number } | null }
      if (conflict.name === 'RoomConflictError') {
        if (conflict.current) {
          dispatch({ type: 'conflict', current: conflict.current.layout, currentRevision: conflict.current.revision })
        } else {
          // The transport did not carry the room; re-read it so the resolution is
          // still an explicit choice rather than a guess.
          try {
            const current = await client.readOwn()
            dispatch({ type: 'conflict', current: current.layout, currentRevision: current.revision })
          } catch {
            dispatch({ type: 'failed', message: 'Your room changed and could not be re-read. Your layout is kept.' })
          }
        }
      } else {
        dispatch({ type: 'failed', message: 'Your room could not be saved. Your layout is kept.' })
      }
    } finally {
      setBusy(false)
    }
  }

  async function upload(input: { readonly preset: PosterPreset; readonly width: number; readonly height: number; readonly bytesBase64: string }) {
    const poster = await client.uploadPoster(input)
    return poster.media
  }

  function place(media: { readonly media_id: string }) {
    const slot = nextSlot(draft)
    if (slot === null) { setMessage('All four poster slots are taken.'); return }
    dispatch({ type: 'place-poster', poster: defaultPosterPlacement(media.media_id, slot) })
    setImporting(false)
  }

  /** Arrow-key movement for the selected furniture, poster or the character. */
  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const selection = state.selected
    if (!selection) return
    const deltas: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }
    const delta = deltas[event.key]
    if (!delta) return
    event.preventDefault()
    if (selection.kind === 'furniture') dispatch({ type: 'move-furniture', index: selection.index, dx: delta[0], dy: delta[1] })
    else if (selection.kind === 'poster') dispatch({ type: 'move-poster', slot: selection.slot, dx: delta[0], dy: delta[1] })
    else dispatch({ type: 'move-character', dx: delta[0], dy: delta[1] })
  }

  return <div className="room-editor" onKeyDown={onKeyDown}>
    <p role="status" aria-live="polite">{state.message ?? message ?? (dirty ? 'Unsaved changes.' : 'All changes saved.')}</p>

    {state.conflict && <section className="room-editor__conflict" role="alert" aria-labelledby={`${id}-conflict`}>
      <h2 id={`${id}-conflict`}>This room changed while you were editing</h2>
      <p>Someone saved revision {state.conflict.currentRevision}. Your layout is still here and has not been discarded.</p>
      <div className="room-editor__actions">
        <button type="button" onClick={() => dispatch({ type: 'keep-draft' })}>Keep my layout</button>
        <button type="button" onClick={() => dispatch({ type: 'use-server' })}>Use the saved room</button>
      </div>
    </section>}

    <fieldset>
      <legend>Wall</legend>
      <div className="room-editor__row">
        {WALL_STYLES.map(wall => <label key={wall}>
          <input type="radio" name={`${id}-wall`} checked={draft.wall === wall} onChange={() => dispatch({ type: 'wall', wall })} />
          {WALL_LABELS[wall]}
        </label>)}
      </div>
    </fieldset>

    <fieldset>
      <legend>Floor</legend>
      <div className="room-editor__row">
        {FLOOR_STYLES.map(floor => <label key={floor}>
          <input type="radio" name={`${id}-floor`} checked={draft.floor === floor} onChange={() => dispatch({ type: 'floor', floor })} />
          {FLOOR_LABELS[floor]}
        </label>)}
      </div>
    </fieldset>

    <fieldset>
      <legend>Furniture ({draft.furniture.length} of {MAX_FURNITURE})</legend>
      <div className="room-editor__row">
        {FURNITURE_ITEMS.map(item => <button key={item} type="button" disabled={draft.furniture.length >= MAX_FURNITURE}
          onClick={() => dispatch({ type: 'add-furniture', item })}>Add {ITEM_LABELS[item].toLowerCase()}</button>)}
      </div>
      <ol className="room-editor__list" ref={list} aria-label="Placed furniture">
        {draft.furniture.map((placement, index) => <li key={`${placement.item_id}-${index}`}
          data-selected={state.selected?.kind === 'furniture' && state.selected.index === index}>
          <button type="button" aria-pressed={state.selected?.kind === 'furniture' && state.selected.index === index}
            onClick={() => dispatch({ type: 'select', selection: { kind: 'furniture', index } })}>
            {ITEM_LABELS[placement.item_id]} at {placement.cell_x},{placement.cell_y} layer {placement.layer}
          </button>
          <span role="group" aria-label={`Move ${ITEM_LABELS[placement.item_id]}`}>
            <button type="button" aria-label="Left" onClick={() => dispatch({ type: 'move-furniture', index, dx: -1, dy: 0 })}>Left</button>
            <button type="button" aria-label="Right" onClick={() => dispatch({ type: 'move-furniture', index, dx: 1, dy: 0 })}>Right</button>
            <button type="button" aria-label="Up" onClick={() => dispatch({ type: 'move-furniture', index, dx: 0, dy: -1 })}>Up</button>
            <button type="button" aria-label="Down" onClick={() => dispatch({ type: 'move-furniture', index, dx: 0, dy: 1 })}>Down</button>
          </span>
          <button type="button" aria-label={`Send ${ITEM_LABELS[placement.item_id]} back a layer`} disabled={placement.layer === 0}
            onClick={() => dispatch({ type: 'layer-furniture', index, delta: -1 })}>Back</button>
          <button type="button" aria-label={`Bring ${ITEM_LABELS[placement.item_id]} forward a layer`} disabled={placement.layer === 3}
            onClick={() => dispatch({ type: 'layer-furniture', index, delta: 1 })}>Forward</button>
          <button type="button" aria-label={`Remove ${ITEM_LABELS[placement.item_id]}`} onClick={() => dispatch({ type: 'remove-furniture', index })}>Remove</button>
        </li>)}
      </ol>
    </fieldset>

    <fieldset>
      <legend>Posters ({draft.posters.length} of {MAX_POSTERS})</legend>
      <div className="room-editor__row">
        <button type="button" disabled={draft.posters.length >= MAX_POSTERS} onClick={() => setImporting(true)}>Add a poster</button>
      </div>
      <ol className="room-editor__list" aria-label="Placed posters">
        {draft.posters.map(poster => <li key={poster.slot} data-selected={state.selected?.kind === 'poster' && state.selected.slot === poster.slot}>
          <button type="button" aria-pressed={state.selected?.kind === 'poster' && state.selected.slot === poster.slot}
            onClick={() => dispatch({ type: 'select', selection: { kind: 'poster', slot: poster.slot } })}>
            Poster in slot {poster.slot + 1} at {poster.x},{poster.y}
          </button>
          <span role="group" aria-label={`Move poster in slot ${poster.slot + 1}`}>
            <button type="button" aria-label="Left" onClick={() => dispatch({ type: 'move-poster', slot: poster.slot, dx: -1, dy: 0 })}>Left</button>
            <button type="button" aria-label="Right" onClick={() => dispatch({ type: 'move-poster', slot: poster.slot, dx: 1, dy: 0 })}>Right</button>
            <button type="button" aria-label="Up" onClick={() => dispatch({ type: 'move-poster', slot: poster.slot, dx: 0, dy: -1 })}>Up</button>
            <button type="button" aria-label="Down" onClick={() => dispatch({ type: 'move-poster', slot: poster.slot, dx: 0, dy: 1 })}>Down</button>
          </span>
          <button type="button" aria-label={`Remove poster in slot ${poster.slot + 1}`} onClick={() => dispatch({ type: 'remove-poster', slot: poster.slot })}>Remove</button>
        </li>)}
      </ol>
      {importing && <PosterImporter converter={converter} upload={upload} place={place}
        full={draft.posters.length >= MAX_POSTERS} onClose={() => setImporting(false)} />}
    </fieldset>

    <fieldset>
      <legend>Character</legend>
      <div className="room-editor__row">
        <button type="button" onClick={() => dispatch({ type: 'select', selection: { kind: 'character' } })}>Select character</button>
        <span role="group" aria-label="Move character">
          <button type="button" aria-label="Left" onClick={() => dispatch({ type: 'move-character', dx: -1, dy: 0 })}>Left</button>
          <button type="button" aria-label="Right" onClick={() => dispatch({ type: 'move-character', dx: 1, dy: 0 })}>Right</button>
          <button type="button" aria-label="Up" onClick={() => dispatch({ type: 'move-character', dx: 0, dy: -1 })}>Up</button>
          <button type="button" aria-label="Down" onClick={() => dispatch({ type: 'move-character', dx: 0, dy: 1 })}>Down</button>
        </span>
        <button type="button" aria-pressed={draft.character.facing === 'LEFT'} onClick={() => dispatch({ type: 'face', facing: 'LEFT' })}>Face left</button>
        <button type="button" aria-pressed={draft.character.facing === 'RIGHT'} onClick={() => dispatch({ type: 'face', facing: 'RIGHT' })}>Face right</button>
      </div>
      <p>The character stands in your room. It never shows whether you are online.</p>
    </fieldset>

    <fieldset>
      <legend>Who can see this room</legend>
      <div className="room-editor__row">
        <label><input type="radio" name={`${id}-visibility`} checked={visibility === 'PUBLIC'} onChange={() => onVisibility('PUBLIC')} /> Public</label>
        <label><input type="radio" name={`${id}-visibility`} checked={visibility === 'PRIVATE'} onChange={() => onVisibility('PRIVATE')} /> Private</label>
      </div>
      <p>Room visibility is separate from your intro post audience. Changing one never changes the other.</p>
      <p className="room-editor__note">Room revision {revision}</p>
    </fieldset>

    <div className="room-editor__actions">
      <button type="button" onClick={() => void save()} disabled={busy || !dirty}>Save room</button>
      <button type="button" onClick={() => dispatch({ type: 'loaded', layout: state.saved ?? draft, revision: state.revision })} disabled={busy || !dirty}>Discard changes</button>
    </div>
  </div>
}

function nextSlot(layout: RoomLayout): number | null {
  for (let slot = 0; slot < MAX_POSTERS; slot++) {
    if (!layout.posters.some(poster => poster.slot === slot)) return slot
  }
  return null
}

/** The grid size, exported so the screen and the editor cannot disagree. */
export { GRID_CELLS }

/**
 * A hook that keeps the editor's draft in sync with a freshly loaded room without
 * overwriting unsaved work on a re-render.
 */
export function useRoomDraft(loaded: { readonly layout: RoomLayout; readonly revision: number } | null) {
  const [state, setState] = useState<EditorState>(() => ({
    draft: { version: 1, wall: 'plaster', floor: 'wood', furniture: [], posters: [], character: { cell_x: 16, cell_y: 24, facing: 'RIGHT' } },
    saved: null, revision: 1, message: null, selected: null, conflict: null, saving: false,
  }))
  const applied = useRef<number | null>(null)
  useEffect(() => {
    if (!loaded || applied.current === loaded.revision) return
    applied.current = loaded.revision
    setState(current => reduce(current, { type: 'loaded', layout: loaded.layout, revision: loaded.revision }))
  }, [loaded])
  return [state, setState] as const
}
