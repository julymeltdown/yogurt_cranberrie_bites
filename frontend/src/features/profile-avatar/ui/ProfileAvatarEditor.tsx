import { useEffect, useId, useState, useSyncExternalStore } from 'react'
import type { MotionDirection, MotionLayer, MotionSelection } from '../../../entities/emo-avatar/art'
import { canonicalJson } from '../../../world/map/json'
import type { ProfileAvatarEditorController, MotionNotice } from '../model/controller'
import { MotionBodyPalette } from './MotionBodyPalette'
import { MotionOption } from './MotionOption'
import { MotionView } from './MotionView'
import './profile-avatar.css'

/**
 * WP-08 T-704: the profile avatar editor.
 *
 * This is the concert `MotionEditor` re-pointed at the profile:
 *
 * * the window is the profile's own frame, not the concert window chrome;
 * * the copy no longer talks about a concert, a World or an event;
 * * there is **no event gate**: the editor renders whenever it has an
 *   account-scoped controller, and a controller that is not for the current
 *   account renders the explicit unavailable state instead of the previous
 *   account's character.
 *
 * What is kept: preset/component/colour selection, the direction and walk
 * previews, the draft-until-save rule, and the conflict reconciliation.
 */
const messages: Record<Exclude<MotionNotice, null>, string> = {
  'load-failed': 'Your saved character could not be verified. Reload to try again.',
  'art-unavailable': 'Character artwork is unavailable. Reload the collection to try again.',
  'invalid-selection': 'That choice is unavailable in this collection.', saved: 'Character saved.',
  'save-failed': 'Your character was not saved. Your draft is still here.',
  conflict: 'Your saved character changed elsewhere. Check the latest saved character before saving.',
  uncertain: 'The save result is unknown. Check the saved character before another save.',
  'review-server': 'Latest saved character loaded. Choose whether to keep your draft or use the saved character.',
  'saved-refresh-needed': 'Character saved. Reload your profile to refresh its display.',
}
const equal = (a: unknown, b: unknown) => canonicalJson(a as Record<string, unknown>) === canonicalJson(b as Record<string, unknown>)
const choicesPerPage = 4
const layerLabels: Record<MotionLayer, string> = { body: 'Skin / body', hair: 'Hair', eyes: 'Eyes', outfit: 'Outfit', accessory: 'Accessory' }
const label = (key: string) => `${key.split('-')[0]} ${Number(key.split('-').at(-1)) + 1}`
const categoryPaths: Record<MotionLayer, string> = {
  hair: 'M4 19V11a8 8 0 0 1 16 0v8l-4-2v-6l-4-4-3 5H7v5z',
  outfit: 'M8 4 3 7l2 5 3-1v9h8v-9l3 1 2-5-5-3c0 4-8 4-8 0Z',
  accessory: 'M3 10h5l2 2h4l2-2h5v5l-2 2h-3l-2-5h-4l-2 5H5l-2-2Zm0 0L1 8m20 2 2-2',
  body: 'M12 3a4 4 0 1 1 0 8 4 4 0 0 1 0-8Zm-7 18v-3a7 7 0 0 1 14 0v3',
  eyes: 'M2 12s4-6 10-6 10 6 10 6-4 6-10 6S2 12 2 12Zm10-3a3 3 0 1 1 0 6 3 3 0 0 1 0-6Z',
}

export function ProfileAvatarEditor<C>({ controller }: { readonly controller: ProfileAvatarEditorController<C> & { readonly lifetime?: 'owner' } }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), id = useId()
  const [direction, setDirection] = useState<MotionDirection>('down'), [walking, setWalking] = useState(false)
  const [category, setCategory] = useState<MotionLayer>('hair'), [page, setPage] = useState(0)
  useEffect(() => { controller.start(); return () => { if (controller.lifetime !== 'owner') controller.stop() } }, [controller])
  const busy = ['loading', 'saving', 'reconciling'].includes(state.phase), editable = state.phase === 'ready' && !!state.draft
  const draft = state.draft, catalog = state.catalog
  const selected = draft ? controller.runtime.authority.selection(draft) : null
  const dirty = !!draft && !!state.persisted && !equal(draft, state.persisted.config)
  const items = catalog?.items.filter(item => item.kind === category) ?? [], last = Math.max(0, Math.ceil(items.length / choicesPerPage) - 1), current = Math.min(page, last)
  const chooseItem = (kind: MotionLayer, key: string | null) => {
    if (selected?.mode !== 'wardrobe') return
    if (kind === category) setPage(Math.floor(Math.max(0, items.findIndex(item => item.key === key)) / choicesPerPage))
    void controller.choose({ ...selected, [kind]: key } as MotionSelection)
  }
  const dressUp = () => {
    if (!catalog || selected?.mode === 'wardrobe') return
    setPage(0)
    void controller.choose({ mode: 'wardrobe', body: catalog.items.find(i => i.kind === 'body')!.key,
      hair: catalog.items.find(i => i.kind === 'hair')!.key, eyes: catalog.items.find(i => i.kind === 'eyes')!.key,
      outfit: catalog.items.find(i => i.kind === 'outfit')!.key, accessory: null })
  }
  return <section className="mcr-profile-avatar" aria-label="Character editor" aria-busy={busy}>
    <header className="mcr-profile-avatar__header">
      <h1>Your character</h1>
      <p>This is the character shown on your profile.</p>
    </header>
    <div className="mcr-profile-avatar__actions">
      <button type="button" aria-label="Random character" disabled={!editable} onClick={() => void controller.random()}>Random</button>
      <button type="button" aria-label="Cancel changes" disabled={!editable || !!state.candidate} onClick={() => controller.reset()}>Cancel</button>
      <button type="button" aria-label="Save character" aria-busy={state.phase === 'saving'} disabled={!editable || !dirty || state.needsReconciliation || controller.runtime.authority.qualification === 'REVIEWED_PRODUCTION' && !controller.runtime.authority.canSave} onClick={() => void controller.save()}>Save</button>
    </div>
    <div className="mcr-profile-avatar__stage">
      <div className="mcr-profile-avatar__figure"><div className="mcr-profile-avatar__preview">
        {draft && !['idle', 'unavailable', 'disposed'].includes(state.phase)
          ? <MotionView runtime={controller.runtime} config={draft} direction={direction} walking={walking} size="fill" label="Draft character preview" />
          : <p>{busy ? 'Loading your character…' : 'Character preview unavailable'}</p>}
      </div>
      <div className="mcr-profile-avatar__pose">
        <label htmlFor={`${id}-direction`}>View</label>
        <select aria-label="View" id={`${id}-direction`} value={direction} onChange={e => setDirection(e.target.value as MotionDirection)}>
          <option value="down">Front</option><option value="left">Left</option><option value="right">Right</option><option value="up">Rear</option>
        </select>
        <button type="button" aria-label={walking ? 'Stop walking' : 'Preview walk'} aria-pressed={walking} disabled={!draft} onClick={() => setWalking(!walking)}>{walking ? 'Stop walk' : 'Walk'}</button>
      </div></div>
      {catalog && draft && <div className="mcr-profile-avatar__categories" role="group" aria-label="Clothing categories">
        {(['hair', 'eyes', 'outfit', 'body', 'accessory'] as const).map(kind => <button type="button" key={kind} aria-label={layerLabels[kind]}
          className={['hair', 'eyes', 'outfit'].includes(kind) ? 'mcr-profile-avatar__category--primary' : 'mcr-profile-avatar__category--secondary'}
          disabled={!editable} aria-pressed={selected?.mode === 'wardrobe' && category === kind} onClick={() => {
            setCategory(kind)
            const choices = catalog.items.filter(item => item.kind === kind)
            setPage(selected?.mode === 'wardrobe' ? Math.floor(Math.max(0, choices.findIndex(item => item.key === selected[kind])) / choicesPerPage) : 0)
            if (selected?.mode !== 'wardrobe') dressUp()
          }}><svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" strokeLinecap="round"><path d={categoryPaths[kind]} /></svg>
          <span>{layerLabels[kind]}</span></button>)}
      </div>}
    </div>
    {catalog && draft && <>
      <div className="mcr-profile-avatar__mode" role="group" aria-label="Character mode">
        <button type="button" disabled={!editable} aria-pressed={selected?.mode === 'preset'} onClick={() => { if (selected?.mode !== 'preset') controller.selectPreset(catalog.presets[0]!.id) }}>Presets</button>
        <button type="button" disabled={!editable} aria-pressed={selected?.mode === 'wardrobe'} onClick={dressUp}>Dress up</button>
      </div>
      {selected?.mode === 'preset' ? <div className="mcr-profile-avatar__presets" role="group" aria-label="Preset choices">
        {catalog.presets.map(preset => <button key={preset.id} type="button" aria-label={`Choose ${preset.label}`} disabled={!editable} aria-pressed={equal(draft, preset.config)} onClick={() => controller.selectPreset(preset.id)}>
          <MotionView runtime={controller.runtime} config={preset.config} size={64} label={preset.label} /><span>{preset.label}</span>
        </button>)}
      </div> : <>
        <section className="mcr-profile-avatar__options" aria-label={layerLabels[category]}>
          <header><h2>{layerLabels[category]}</h2><span>{items.length} choices</span></header>
          {category === 'body' && <MotionBodyPalette controller={controller} />}
          <div className="mcr-profile-avatar__option-row">
            <button type="button" aria-label={`Previous ${layerLabels[category]} choices`} disabled={current === 0} onClick={() => setPage(current - 1)}>‹</button>
            <div className="mcr-profile-avatar__option-grid">
              {selected?.mode === 'wardrobe' && items.slice(current * choicesPerPage, (current + 1) * choicesPerPage).map(item => <MotionOption key={item.key} runtime={controller.runtime} draft={draft}
                part={{ kind: category, key: item.key }} selection={{ ...selected, [category]: item.key }} label={label(item.key)} pressed={selected[category] === item.key} disabled={!editable} onChoose={() => chooseItem(category, item.key)} />)}
            </div>
            <button type="button" aria-label={`Next ${layerLabels[category]} choices`} disabled={current === last} onClick={() => setPage(current + 1)}>›</button>
          </div>
          {(category === 'outfit' || category === 'accessory') && <button type="button" disabled={!editable} aria-pressed={selected?.mode === 'wardrobe' && selected[category] === null} onClick={() => chooseItem(category, null)}>None</button>}
          <div className="mcr-profile-avatar__select"><label htmlFor={`${id}-item`}>Choose {layerLabels[category].toLowerCase()}</label>
            <select id={`${id}-item`} value={selected?.mode === 'wardrobe' ? selected[category] ?? '' : ''} disabled={!editable} onChange={e => chooseItem(category, e.target.value || null)}>
              {(category === 'outfit' || category === 'accessory') && <option value="">None</option>}
              {items.map(item => <option key={item.key} value={item.key}>{label(item.key)}</option>)}
            </select>
          </div>
        </section>
      </>}
    </>}
    <div className="mcr-profile-avatar__status"><p role="status" aria-live="polite">{busy ? state.phase === 'saving' ? 'Saving character…' : 'Loading your character…' : state.notice ? messages[state.notice] : 'Changes are a draft until you save.'}</p>
      {(state.notice === 'art-unavailable' || state.notice === 'load-failed') && <button type="button" disabled={busy} onClick={() => state.notice === 'art-unavailable' ? controller.retry() : void controller.reload()}>Reload character</button>}
      {state.needsReconciliation && !state.candidate && <button type="button" disabled={busy} onClick={() => void controller.reload()}>Check latest saved character</button>}
      {state.candidate && <section aria-label="Saved character reconciliation"><h2>Saved character · revision {state.candidate.revision}</h2>
        <button type="button" onClick={() => controller.resolveConflict('keep-draft')}>Keep my draft</button>
        <button type="button" onClick={() => controller.resolveConflict('use-server')}>Use saved character</button>
      </section>}
    </div>
  </section>
}
