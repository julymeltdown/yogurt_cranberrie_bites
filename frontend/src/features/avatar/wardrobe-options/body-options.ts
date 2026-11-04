import type { MotionItem, MotionSelection } from '../../../entities/avatar/runtime/motion-adoption/art'

export interface MotionBodyOption {
  readonly key: string
  readonly label: string
  readonly color: string | null
}
// Reviewed C06 composition.hand_palette_color_index and the existing compositor both use 2.
// This is one exact supplied RGB triplet, never an inferred shade or image tint.
const representativeIndex = 2
export function motionBodyOptions(items: readonly MotionItem[]): readonly MotionBodyOption[] {
  const body = items.filter(item => item.kind === 'body'), seen = new Set<string>()
  if (body.some(item => !item.key || seen.has(item.key) || !seen.add(item.key))) return Object.freeze([])
  return Object.freeze(body.map((item, index) => {
    const colors = item.skinPalette?.colors
    const valid = Array.isArray(colors) && colors.length === 4 && colors.every(rgb => Array.isArray(rgb) && rgb.length === 3
      && rgb.every(channel => Number.isInteger(channel) && channel >= 0 && channel <= 255))
    const rgb = valid ? colors[representativeIndex] : null
    return Object.freeze({ key: item.key, label: `Skin tone ${index + 1}`, color: rgb ? `rgb(${rgb.join(', ')})` : null })
  }))
}
/** Change only a real, currently available body key in an existing wardrobe selection. */
export function motionBodySelection(selection: MotionSelection, key: string, options: readonly MotionBodyOption[]): MotionSelection | null {
  if (selection.mode !== 'wardrobe' || !options.some(option => option.key === key && option.color !== null)) return null
  return Object.freeze({ ...selection, body: key })
}
