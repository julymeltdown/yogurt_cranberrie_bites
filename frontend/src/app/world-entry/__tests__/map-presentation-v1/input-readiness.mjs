// Necessary source-input checks only. This never grants production admission.
export function inspectInputReadiness(input) {
  const reasons = []
  if (input?.event?.phase !== 'OPEN') reasons.push('EVENT_NOT_OPEN')
  if (input?.capabilities?.world?.available !== true) reasons.push('WORLD_NOT_AVAILABLE')
  else if (input.capabilities.world.reason !== null) reasons.push('INVALID_POSITIVE_WORLD_REASON')
  if (input?.map_manifest?.map_id !== 'mcr_reference_scene_r3' || input?.map_manifest?.map_revision !== 4)
    reasons.push('NOT_MAP4_INPUT')
  return Object.freeze({
    status: reasons.length ? 'INPUT_NOT_READY' : 'INPUT_CANDIDATE',
    productionAdmitted: false,
    reasons: Object.freeze(reasons),
    worldReason: typeof input?.capabilities?.world?.reason === 'string' ? input.capabilities.world.reason : null,
    limitation: 'Enabled flags and a Map4 identity are necessary only. Full authenticated C03 validation stays in memory; retained runtime evidence is a public-resource projection, never a private body or its digest. Normal contract/hash/admission validation still applies.',
  })
}
