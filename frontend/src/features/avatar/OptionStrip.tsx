import { useId, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import { layers, sameConfig, type AvatarConfig, type Layer } from './contract'
import { OPTION_PREVIEW_LIMIT, type AvatarEditorController, type OptionPreview } from './controller'

const label = (id: string) => id.replaceAll('_', ' ')
interface Choice { readonly id: string; readonly text: string; readonly config: AvatarConfig; readonly selected: boolean; select(): boolean }
function OutfitCanvas({ canvas }: { readonly canvas: HTMLCanvasElement | null }) {
  const host = useRef<HTMLSpanElement>(null)
  useLayoutEffect(() => {
    if (!canvas || !host.current) return
    host.current.replaceChildren(canvas)
    return () => { canvas.remove() }
  }, [canvas])
  return <span className="mcr-avatar-option-pixels" aria-hidden="true" ref={host} />
}

/** Full-outfit candidates only. No thumbnail URLs, isolated-asset claims or separate renderer. */
export function OptionStrip({ controller, category }: { readonly controller: AvatarEditorController; readonly category: string }) {
  const state = useSyncExternalStore(controller.subscribe, controller.read, controller.read), id = useId()
  const [page, setPage] = useState(0)
  const choices = useMemo<readonly Choice[]>(() => {
    const draft = state.draft, catalog = state.catalog
    if (!draft || !catalog) return []
    if (category === 'presets') return catalog.presets.map(preset => ({ id: preset.id, text: label(preset.id), config: preset.config,
      selected: sameConfig(draft, preset.config), select: () => controller.selectPreset(preset.id) }))
    if (category === 'tone') return catalog.tones.map(tone => ({ id: tone, text: label(tone), config: { ...draft, skin_tone_id: tone },
      selected: draft.skin_tone_id === tone, select: () => controller.selectTone(tone) }))
    if (!layers.includes(category as Layer)) return []
    const layer = category as Layer, items = catalog.items.filter(item => item.layer === layer).map(item => ({ id: item.id, text: label(item.id), config: { ...draft, item_ids: { ...draft.item_ids, [layer]: item.id } },
      selected: draft.item_ids[layer] === item.id, select: () => controller.selectItem(layer, item.id) }))
    if (layer === 'accessory') items.unshift({ id: '@none', text: 'None', config: { ...draft, item_ids: { ...draft.item_ids, accessory: null } },
      selected: draft.item_ids.accessory === null, select: () => controller.selectItem('accessory', null) })
    return items
  }, [state.draft, state.catalog, category, controller])
  const last = Math.max(0, Math.ceil(choices.length / OPTION_PREVIEW_LIMIT) - 1), currentPage = Math.min(page, last)
  const visible = useMemo(() => choices.slice(currentPage * OPTION_PREVIEW_LIMIT, (currentPage + 1) * OPTION_PREVIEW_LIMIT), [choices, currentPage])
  const editable = state.phase === 'ready' && state.draft !== null
  const [previews, setPreviews] = useState<{ source: readonly Choice[]; items: readonly OptionPreview[] }>({ source: [], items: [] })
  useLayoutEffect(() => {
    if (!editable || !visible.length) return
    let active = true
    setPreviews({ source: visible, items: visible.map(() => ({ phase: 'loading', canvas: null })) })
    const release = controller.acquireOptionPreviews(visible.map(choice => choice.config), (index, result) => {
      if (active) setPreviews(previous => ({ source: visible, items: visible.map((_, i) => i === index ? result : previous.source === visible ? previous.items[i]! : { phase: 'loading', canvas: null }) }))
    })
    return () => { active = false; release() }
  }, [controller, visible, editable])
  if (!choices.length) return null
  return <section className="mcr-avatar-option-strip" aria-labelledby={`${id}-title`}>
    <h2 id={`${id}-title`}>{category === 'tone' ? 'Tones' : label(category)} · outfit previews</h2>
    <p id={`${id}-description`}>Full outfit with each choice. Nothing is saved until you save your outfit.</p>
    <div className="mcr-avatar-option-grid" role="group" aria-label={`${label(category)} outfit choices`} aria-describedby={`${id}-description`}>
      {visible.map((choice, index) => {
        const result = editable && previews.source === visible ? previews.items[index] : undefined
        return <button key={choice.id} className="mcr-avatar-option" type="button" aria-label={`Try ${choice.text} · full outfit`} aria-pressed={choice.selected}
          disabled={!editable || result?.phase !== 'ready' || !controller.canSelect(choice.config)} onClick={() => choice.select()}>
          <OutfitCanvas canvas={result?.canvas ?? null} /><span className="mcr-avatar-option-name" title={choice.id}>{choice.text}</span>
          {result?.phase !== 'ready' && <span className="mcr-avatar-option-state">{result?.phase === 'unavailable' ? 'Unavailable' : 'Loading…'}</span>}
        </button>
      })}
    </div>
    {choices.length > OPTION_PREVIEW_LIMIT && <div className="mcr-avatar-option-pages" role="group" aria-label="Outfit choice pages">
      <button type="button" disabled={currentPage === 0} onClick={() => setPage(currentPage - 1)} aria-label="Previous outfit choices">‹</button>
      <span aria-live="polite">{currentPage * OPTION_PREVIEW_LIMIT + 1}–{Math.min((currentPage + 1) * OPTION_PREVIEW_LIMIT, choices.length)} of {choices.length}</span>
      <button type="button" disabled={currentPage === last} onClick={() => setPage(currentPage + 1)} aria-label="Next outfit choices">›</button>
    </div>}
  </section>
}
