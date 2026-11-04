import { useEffect, useId, useState, useSyncExternalStore } from 'react'
import { ConcertAction, ConcertWindow } from '../../../shared/ui/concert-window'
import { canonicalJson } from '../../../world/map/json'
import type { MotionDirection, MotionLayer, MotionSelection } from '../../../entities/avatar/runtime/motion-adoption/art'
import type { MotionEditorController, MotionNotice } from './controller'
import { MotionView } from './MotionView'
import { MotionOption } from './MotionOption'
import './motion.css'

const messages: Record<Exclude<MotionNotice, null>, string> = {
  'load-failed': 'The saved outfit could not be verified. Reload to try again.',
  'art-unavailable': 'Character artwork is unavailable. Reload the collection to try again.',
  'invalid-selection': 'That choice is unavailable in this collection.', saved: 'Outfit saved.',
  'save-failed': 'Your outfit was not saved. Your draft is still here.',
  conflict: 'Your saved outfit changed elsewhere. Check the latest saved outfit before saving.',
  uncertain: 'The save result is unknown. Check the saved outfit before another save.',
  'review-server': 'Latest saved outfit loaded. Choose whether to keep your draft or use the saved outfit.',
  'saved-refresh-needed': 'Outfit saved. Reload World and Profile to refresh their display.',
}
const equal = (a: unknown, b: unknown) => canonicalJson(a as Record<string, unknown>) === canonicalJson(b as Record<string, unknown>)
const layerLabels: Record<MotionLayer, string> = { body: 'Skin / body', hair: 'Hair', eyes: 'Eyes', outfit: 'Outfit', accessory: 'Accessory' }
const label = (key: string) => `${key.split('-')[0]} ${Number(key.split('-').at(-1)) + 1}`
const categoryPaths: Record<MotionLayer, string> = {
  hair: 'M4 19V11a8 8 0 0 1 16 0v8l-4-2v-6l-4-4-3 5H7v5z',
  outfit: 'M8 4 3 7l2 5 3-1v9h8v-9l3 1 2-5-5-3c0 4-8 4-8 0Z',
  accessory: 'M3 10h5l2 2h4l2-2h5v5l-2 2h-3l-2-5h-4l-2 5H5l-2-2Zm0 0L1 8m20 2 2-2',
  body: 'M12 3a4 4 0 1 1 0 8 4 4 0 0 1 0-8Zm-7 18v-3a7 7 0 0 1 14 0v3',
  eyes: 'M2 12s4-6 10-6 10 6 10 6-4 6-10 6S2 12 2 12Zm10-3a3 3 0 1 1 0 6 3 3 0 0 1 0-6Z',
}
export function MotionEditor<C>({ controller }: { readonly controller: MotionEditorController<C> }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), id = useId()
  const [direction, setDirection] = useState<MotionDirection>('down'), [walking, setWalking] = useState(false)
  const [category, setCategory] = useState<MotionLayer>('hair'), [page, setPage] = useState(0)
  useEffect(() => { controller.start(); return () => controller.stop() }, [controller])
  const busy = ['loading', 'saving', 'reconciling'].includes(state.phase), editable = state.phase === 'ready' && !!state.draft
  const draft = state.draft, catalog = state.catalog
  const selected = draft ? controller.runtime.authority.selection(draft) : null
  const dirty = !!draft && !!state.persisted && !equal(draft, state.persisted.config)
  const items = catalog?.items.filter(item => item.kind === category) ?? [], last = Math.max(0, Math.ceil(items.length / 3) - 1), current = Math.min(page, last)
  const chooseItem = (kind: MotionLayer, key: string | null) => {
    if (selected?.mode !== 'wardrobe') return
    if (kind === category) setPage(Math.floor(Math.max(0, items.findIndex(item => item.key === key)) / 3))
    void controller.choose({ ...selected, [kind]: key } as MotionSelection)
  }
  const dressUp = () => {
    if (!catalog || selected?.mode === 'wardrobe') return
    setPage(0)
    void controller.choose({ mode: 'wardrobe', body: catalog.items.find(i => i.kind === 'body')!.key,
      hair: catalog.items.find(i => i.kind === 'hair')!.key, eyes: catalog.items.find(i => i.kind === 'eyes')!.key,
      outfit: catalog.items.find(i => i.kind === 'outfit')!.key, accessory: null })
  }
  return <section className="mcr-motion-editor" aria-label="Avatar editor" aria-busy={busy}>
    <ConcertWindow title="CUSTOMIZE" headingLevel={1} icon="me" footer={<div className="mcr-motion-actions">
      <ConcertAction tone="paper" aria-label="Random character" disabled={!editable} onClick={() => void controller.random()}>Random</ConcertAction>
      <ConcertAction tone="paper" aria-label="Reset draft" disabled={!editable || !!state.candidate} onClick={() => controller.reset()}>Reset</ConcertAction>
      <ConcertAction aria-label="Save outfit" pending={state.phase === 'saving'} disabled={!editable || !dirty || state.needsReconciliation} onClick={() => void controller.save()}>Save</ConcertAction>
    </div>}>
      <div className="mcr-motion-stage">
      <div className="mcr-motion-figure"><div className="mcr-motion-preview">
        {draft && !['idle', 'unavailable', 'disposed'].includes(state.phase)
          ? <MotionView runtime={controller.runtime} config={draft} direction={direction} walking={walking} size="fill" label="Draft outfit preview" />
          : <p>{busy ? 'Loading your character…' : 'Character preview unavailable'}</p>}
      </div>
      <div className="mcr-motion-pose">
        <select aria-label="View" id={`${id}-direction`} value={direction} onChange={e => setDirection(e.target.value as MotionDirection)}>
          <option value="down">Front</option><option value="left">Left</option><option value="right">Right</option><option value="up">Rear</option>
        </select>
        <button type="button" aria-label={walking ? 'Stop walking' : 'Preview walk'} aria-pressed={walking} disabled={!draft} onClick={() => setWalking(!walking)}>{walking ? 'Stop walk' : 'Walk'}</button>
      </div></div>
      {catalog && draft && <div className="mcr-motion-categories" role="group" aria-label="Clothing categories">
        {(['hair', 'outfit', 'accessory', 'body', 'eyes'] as const).map(kind => <button type="button" key={kind} aria-label={layerLabels[kind]}
          disabled={!editable} aria-pressed={selected?.mode === 'wardrobe' && category === kind} onClick={() => {
            setCategory(kind)
            const choices = catalog.items.filter(item => item.kind === kind)
            setPage(selected?.mode === 'wardrobe' ? Math.floor(Math.max(0, choices.findIndex(item => item.key === selected[kind])) / 3) : 0)
            if (selected?.mode !== 'wardrobe') dressUp()
          }}><svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" strokeLinecap="round"><path d={categoryPaths[kind]} /></svg>
          <span>{layerLabels[kind]}</span></button>)}
      </div>}
      </div>
      {catalog && draft && <>
        <div className="mcr-motion-mode" role="group" aria-label="Character mode">
          <button type="button" disabled={!editable} aria-pressed={selected?.mode === 'preset'} onClick={() => { if (selected?.mode !== 'preset') controller.selectPreset(catalog.presets[0]!.id) }}>Presets</button>
          <button type="button" disabled={!editable} aria-pressed={selected?.mode === 'wardrobe'} onClick={dressUp}>Dress up</button>
        </div>
        {selected?.mode === 'preset' ? <div className="mcr-motion-presets" role="group" aria-label="Preset choices">
          {catalog.presets.map(preset => <button key={preset.id} type="button" aria-label={`Choose ${preset.label}`} disabled={!editable} aria-pressed={equal(draft, preset.config)} onClick={() => controller.selectPreset(preset.id)}>
            <MotionView runtime={controller.runtime} config={preset.config} size={64} label={preset.label} /><span>{preset.label}</span>
          </button>)}
        </div> : <>
          <section className="mcr-motion-options" aria-label={layerLabels[category]}>
            <header><h2>{layerLabels[category]}</h2><span>{items.length} choices</span></header>
            <div className="mcr-motion-option-row">
              <button type="button" aria-label={`Previous ${layerLabels[category]} choices`} disabled={current === 0} onClick={() => setPage(current - 1)}>‹</button>
              <div className="mcr-motion-option-grid">
                {selected?.mode === 'wardrobe' && items.slice(current * 3, current * 3 + 3).map(item => <MotionOption key={item.key} runtime={controller.runtime} draft={draft}
                  selection={{ ...selected, [category]: item.key }} label={label(item.key)} pressed={selected[category] === item.key} disabled={!editable} onChoose={() => chooseItem(category, item.key)} />)}
              </div>
              <button type="button" aria-label={`Next ${layerLabels[category]} choices`} disabled={current === last} onClick={() => setPage(current + 1)}>›</button>
            </div>
            {(category === 'outfit' || category === 'accessory') && <button type="button" disabled={!editable} aria-pressed={selected?.mode === 'wardrobe' && selected[category] === null} onClick={() => chooseItem(category, null)}>None</button>}
            <div className="mcr-motion-select"><label htmlFor={`${id}-item`}>Choose {layerLabels[category].toLowerCase()}</label>
              <select id={`${id}-item`} value={selected?.mode === 'wardrobe' ? selected[category] ?? '' : ''} disabled={!editable} onChange={e => chooseItem(category, e.target.value || null)}>
                {(category === 'outfit' || category === 'accessory') && <option value="">None</option>}
                {items.map(item => <option key={item.key} value={item.key}>{label(item.key)}</option>)}
              </select>
            </div>
          </section>
        </>}
      </>}
      <div className="mcr-motion-status"><p role="status" aria-live="polite">{busy ? state.phase === 'saving' ? 'Saving outfit…' : 'Loading your outfit…' : state.notice ? messages[state.notice] : 'Changes are a draft until you save.'}</p>
        {(state.notice === 'art-unavailable' || state.notice === 'load-failed') && <button type="button" disabled={busy} onClick={() => state.notice === 'art-unavailable' ? controller.retry() : void controller.reload()}>Reload character</button>}
        {state.needsReconciliation && !state.candidate && <button type="button" disabled={busy} onClick={() => void controller.reload()}>Check latest saved outfit</button>}
        {state.candidate && <section aria-label="Saved outfit reconciliation"><h2>Saved outfit · revision {state.candidate.revision}</h2>
          <button type="button" onClick={() => controller.resolveConflict('keep-draft')}>Keep my draft</button>
          <button type="button" onClick={() => controller.resolveConflict('use-server')}>Use saved outfit</button>
        </section>}
      </div>
    </ConcertWindow>
  </section>
}
