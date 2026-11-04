/** Product semantics are authored by the consumer, never taken from decorative art. */
export const compositionVersion = 'map-roof-street-derived-v2'
export const serviceSemantics = Object.freeze(([
  { key: 'main_stage', label: 'MAIN STAGE', icon: 'stage', intent: null, path: null },
  { key: 'plaza', label: 'PLAZA', icon: null, intent: null, path: null },
  { key: 'wall', label: 'WALL', icon: 'wall', intent: 'wall', path: '/wall' },
  { key: 'feed', label: 'FEED', icon: 'feed', intent: 'feed', path: '/feed' },
  { key: 'meet', label: 'MEET', icon: 'people', intent: 'meet', path: '/meet' },
  { key: 'gifts', label: 'GIFTS', icon: 'gift', intent: 'gifts', path: '/gifts' },
] as const).map(row => Object.freeze({ ...row, placeId: null, interactionId: null })))
export type ServiceKey = typeof serviceSemantics[number]['key']
export const canonicalCells = Object.freeze({ stage_info: { x: 30, y: 24 }, park_info: { x: 38, y: 27 },
  gifts_info: { x: 13, y: 48 }, wall_info: { x: 14, y: 61 } })
export type CanonicalInteraction = keyof typeof canonicalCells
