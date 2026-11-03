import { sameConfig } from '../contract'
import type { AvatarEditorController } from '../controller'

/** A draft-only preset choice through the existing controller; no new command or inventory. */
export function randomCandidates(controller: AvatarEditorController) {
  const state = controller.read()
  if (state.phase !== 'ready' || !state.draft) return []
  return state.catalog?.presets.filter(preset => controller.canSelect(preset.config) && !sameConfig(preset.config, state.draft!)) ?? []
}
export function randomizeDraft(controller: AvatarEditorController) {
  const candidates = randomCandidates(controller)
  if (!candidates.length) return false
  return controller.selectPreset(candidates[Math.floor(Math.random() * candidates.length)]!.id)
}
