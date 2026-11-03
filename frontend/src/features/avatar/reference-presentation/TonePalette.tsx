import type { AvatarEditorController } from '../controller'

/** Supplied by the owning reviewed manifest; never infer colors from IDs or source JPEGs. */
export interface ToneColor { readonly id: string; readonly color: string }
export function TonePalette({ controller, palette = [] }: { readonly controller: AvatarEditorController; readonly palette?: readonly ToneColor[] | undefined }) {
  const state = controller.read(), seen = new Set<string>()
  const choices = palette.filter(tone => state.catalog?.tones.includes(tone.id) && /^#[0-9a-f]{6}$/i.test(tone.color) && !seen.has(tone.id) && !!seen.add(tone.id))
  if (!choices.length || !state.draft) return null
  return <div className="mcr-avatar-palette" role="group" aria-label="Skin tone colors">
    {choices.map(tone => <button type="button" key={tone.id} aria-label={`Skin tone ${tone.id.replaceAll('_', ' ')}`} title={tone.id}
      aria-pressed={state.draft!.skin_tone_id === tone.id} disabled={state.phase !== 'ready' || !controller.canSelect({ ...state.draft!, skin_tone_id: tone.id })}
      onClick={() => controller.selectTone(tone.id)}><span style={{ backgroundColor: tone.color }} /></button>)}
  </div>
}
