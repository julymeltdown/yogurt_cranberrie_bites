import { useSyncExternalStore } from 'react'
import type { MotionEditorController } from '../motion-adoption/controller'
import type { MotionSelection } from '../../../entities/avatar/runtime/motion-adoption/art'
import { motionBodyOptions, motionBodySelection } from '../wardrobe-options/body-options'
import './body-palette.css'

/** Pure view of the Account-owned editor. Never starts, stops or disposes a borrowed owner. */
export function MotionBodyPalette<C>({ controller }: { readonly controller: MotionEditorController<C> }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read)
  const options = motionBodyOptions(state.catalog?.items ?? [])
  const current = controller.runtime.isCurrent()
  let selected: MotionSelection | null = null
  try { if (current && state.draft) selected = controller.runtime.authority.selection(state.draft) } catch { /* Fail closed on a withdrawn/invalid draft. */ }
  const editable = current && state.phase === 'ready' && selected?.mode === 'wardrobe'
  function choose(key: string) {
    const fresh = controller.read()
    if (!controller.runtime.isCurrent() || fresh.phase !== 'ready' || !fresh.draft || !fresh.catalog) return
    try {
      const selection = controller.runtime.authority.selection(fresh.draft)
      const next = motionBodySelection(selection, key, motionBodyOptions(fresh.catalog.items))
      if (next) void controller.choose(next)
    } catch { /* Owner validation is authoritative; unavailable choices do not create a new draft. */ }
  }
  if (!current || !selected || !options.length) return <p className="mcr-motion-body-unavailable" role="status">Skin tones are unavailable in this character collection.</p>
  return <fieldset className="mcr-motion-body-palette" disabled={!editable}>
    <legend>Skin tones</legend>
    <div className="mcr-motion-body-palette__choices">
      {options.map(option => <button type="button" key={option.key} aria-label={option.label}
        aria-pressed={selected?.mode === 'wardrobe' && selected.body === option.key} disabled={option.color === null}
        title={option.color ? option.label : `${option.label} unavailable in this collection`}
        data-body-key={option.key} onClick={() => choose(option.key)}>
        {option.color ? <span aria-hidden="true" style={{ backgroundColor: option.color }} /> : <span aria-hidden="true">—</span>}
      </button>)}
    </div>
    {selected?.mode === 'preset' && <p>Choose Dress up to change skin tones.</p>}
    {options.some(option => option.color === null) && <p role="status">Some skin tones are unavailable in this collection.</p>}
  </fieldset>
}
