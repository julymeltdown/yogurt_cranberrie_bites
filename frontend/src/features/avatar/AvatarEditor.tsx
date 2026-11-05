import { useEffect, useId, useRef, useState, useSyncExternalStore } from 'react'
import { layers, sameConfig, type AvatarConfig } from './contract'
import type { AvatarEditorController, EditorNotice } from './controller'
import { OptionStrip } from './OptionStrip'
import { ConcertAction, ConcertWindow } from '../../shared/ui/concert-window'
import { AvatarCategoryTabs } from './parade-ink/AvatarCategoryTabs'
import { randomCandidates, randomizeDraft } from './reference-presentation/actions'
import { TonePalette, type ToneColor } from './reference-presentation/TonePalette'
import './avatar-editor.css'
const messages: Record<Exclude<EditorNotice, null>, string> = {
  'invalid-selection': 'That selection is unavailable in the current avatar collection.',
  'load-failed': 'The saved outfit could not be verified. Reload to try again.',
  'save-failed': 'Your outfit was not saved. Your draft is still here. Try again when access is available.',
  saved: 'Outfit saved.', 'saved-refresh-needed': 'Outfit saved. Reload the world and profile to refresh their display.',
  conflict: 'Your saved outfit changed elsewhere. Your draft is still here. Load the latest outfit before saving.',
  uncertain: 'The save result is unknown. Your draft is still here. Check the latest saved outfit before another save.',
  'review-server': 'Latest saved outfit loaded. Choose whether to keep your draft or use the saved outfit.',
  'preview-unavailable': 'Reviewed avatar artwork is unavailable. Customization will be available when the current collection is ready.',
}
const name = (id: string) => id.replaceAll('_', ' ')
export function AvatarEditor({ controller, tonePalette }: { readonly controller: AvatarEditorController; readonly tonePalette?: readonly ToneColor[] | undefined }) {
  const id = useId()
  const [category, setCategory] = useState('hair')
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), host = useRef<HTMLDivElement>(null)
  useEffect(() => { controller.start(); return () => controller.stop() }, [controller])
  useEffect(() => { const element = host.current, canvas = state.preview.canvas; if (!element || !canvas) return
    element.replaceChildren(canvas); return () => { canvas.remove() }
  }, [state.preview.canvas])
  const busy = ['loading', 'saving', 'reconciling'].includes(state.phase)
  const editable = state.phase === 'ready' && state.draft !== null
  function available(config: AvatarConfig) { return controller.canSelect(config) }
  const categories = state.catalog ? [{ value: 'presets', label: 'Presets', content: <fieldset disabled={!editable}><legend>Presets</legend><div className="mcr-avatar-options">
    {state.catalog.presets.map(preset => <button key={preset.id} type="button" disabled={!available(preset.config)}
      aria-pressed={!!state.draft && sameConfig(state.draft, preset.config)} onClick={() => controller.selectPreset(preset.id)}>{name(preset.id)}</button>)}
  </div></fieldset> }, ...layers.map(layer => ({ value: layer, label: layer.charAt(0).toUpperCase() + layer.slice(1), content: <div className="mcr-avatar-field"><label htmlFor={`${id}-${layer}`}>{name(layer)}</label><select id={`${id}-${layer}`} value={state.draft?.item_ids[layer] ?? ''} disabled={!editable} onChange={event => controller.selectItem(layer, event.target.value || null)}>
    {!state.draft && <option value="">Choose {layer}</option>}
    {layer === 'accessory' && <option value="" disabled={!!state.draft && !available({ ...state.draft, item_ids: { ...state.draft.item_ids, accessory: null } })}>None</option>}
    {state.draft && state.draft.item_ids[layer] !== null && !state.catalog?.items.some(item => item.id === state.draft?.item_ids[layer] && item.layer === layer)
      && <option value={state.draft.item_ids[layer]!} disabled>Saved item unavailable</option>}
    {state.catalog!.items.filter(item => item.layer === layer).map(item => <option key={item.id} value={item.id}
      disabled={!state.draft || !available({ ...state.draft, item_ids: { ...state.draft.item_ids, [layer]: item.id } })}>{name(item.id)}</option>)}
  </select></div> })), { value: 'tone', label: 'Tones', content: <div className="mcr-avatar-field"><label htmlFor={`${id}-tone`}>Skin tone</label><select id={`${id}-tone`} value={state.draft?.skin_tone_id ?? ''} disabled={!editable} onChange={event => controller.selectTone(event.target.value)}>
    {!state.draft && <option value="">Choose skin tone</option>}
    {state.draft && !state.catalog.tones.includes(state.draft.skin_tone_id) && <option value={state.draft.skin_tone_id} disabled>Saved skin tone unavailable</option>}
    {state.catalog.tones.map(tone => <option key={tone} value={tone} disabled={!state.draft || !available({ ...state.draft, skin_tone_id: tone })}>{name(tone)}</option>)}
  </select></div> }] : []
  return <section className="mcr-avatar-editor mcr-avatar-editor--parade-ink" aria-label="Avatar editor" aria-busy={busy}><ConcertWindow title="CUSTOMIZE" headingLevel={1} icon="me" footer={<>
    <div className="mcr-avatar-actions">
      <ConcertAction tone="paper" aria-label="Random outfit" disabled={!randomCandidates(controller).length} onClick={() => randomizeDraft(controller)}>Random</ConcertAction>
      <ConcertAction tone="paper" aria-label="Reset draft" disabled={!editable || !!state.candidate} onClick={() => controller.cancel()}>Reset</ConcertAction>
      <ConcertAction aria-label="Save outfit" pending={state.phase === 'saving'} disabled={!editable || state.needsReconciliation || !state.draft || !state.persisted || !available(state.draft) || sameConfig(state.draft, state.persisted.config)} onClick={() => void controller.save()}>Save</ConcertAction>
    </div>
    {state.needsReconciliation && !state.candidate && <p className="mcr-avatar-note">Reset only restores the local draft. Check the saved outfit before another save.</p>}
    {state.candidate && <p className="mcr-avatar-note">Choose Keep my draft or Use saved outfit before resetting.</p>}
  </>}>
    <p className="mcr-avatar-revision mcr-avatar-sr-only">{state.persisted ? `Saved outfit · revision ${state.persisted.revision}` : 'No verified saved outfit'}</p>
    <div className={`mcr-avatar-workbench${state.catalog ? ' mcr-avatar-workbench--catalog' : ''}`}>
    <div className="mcr-avatar-preview" role="img" aria-label={state.preview.phase === 'ready' ? 'Draft outfit preview' : 'Avatar preview unavailable'}>
      <div ref={host} />
      {state.preview.phase !== 'ready' && <p>{state.preview.phase === 'loading' ? 'Loading preview…' : 'Preview unavailable'}</p>}
    </div>
    {state.catalog && <AvatarCategoryTabs items={['hair', 'eyes', 'outfit', 'body', 'accessory', 'tone', 'presets'].flatMap(value => categories.filter(item => item.value === value)).map(item => ({ ...item, content:
      <>{item.value === category && state.draft && <OptionStrip key={category} controller={controller} category={category} />}
      {item.value === category && <TonePalette controller={controller} palette={tonePalette} />}
      <details className="mcr-avatar-id-options"><summary>Choose by ID</summary>{item.content}</details></>,
    }))} value={category} onChange={setCategory} />}
    </div>
    <div className={`mcr-avatar-status${state.phase === 'ready' && !state.notice && !state.needsReconciliation ? ' mcr-avatar-sr-only' : ''}`}>
    <p role="status" aria-live="polite">{state.phase === 'saving' ? 'Saving outfit…' : state.phase === 'reconciling' ? 'Checking latest saved outfit…'
      : state.phase === 'loading' ? 'Loading your outfit…' : state.notice ? messages[state.notice]
        : state.phase === 'unavailable' || state.phase === 'disposed' ? 'Avatar editing is unavailable for this session.' : 'Changes are a draft until you save.'}</p>
    {state.notice === 'preview-unavailable' && <button type="button" disabled={busy} onClick={() => void controller.retryPreview()}>Reload avatar collection</button>}
    {state.phase !== 'unavailable' && state.phase !== 'disposed' && state.notice === 'load-failed' && <button type="button" disabled={busy} onClick={() => void controller.reload()}>Reload saved outfit</button>}
    {state.needsReconciliation && !state.candidate && <button type="button" disabled={busy} onClick={() => void controller.reload()}>Check latest saved outfit</button>}
    {state.candidate && <section aria-label="Saved outfit reconciliation"><h2>Saved outfit · revision {state.candidate.revision}</h2>
      <p>Keeping your draft uses this revision for your next explicit save.</p>
      <button type="button" disabled={busy} onClick={() => controller.resolveConflict('keep-draft')}>Keep my draft</button>
      <button type="button" disabled={busy} onClick={() => controller.resolveConflict('use-server')}>Use saved outfit</button>
    </section>}
    </div>
  </ConcertWindow>
  </section>
}
