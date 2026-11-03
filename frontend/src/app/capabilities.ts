/** C03 capability projection; availability is not a role or resource authorization grant. */
export const CAPABILITIES = ['world', 'feed', 'meet', 'gifts', 'wall', 'social_sharing', 'mutations'] as const
const REASONS = ['EVENT_READ_ONLY', 'DEPENDENCY_UNAVAILABLE', 'PROVIDER_UNAVAILABLE', 'FEATURE_DISABLED', 'NOT_ELIGIBLE'] as const
export type Capability = { readonly available: true; readonly reason: null }
  | { readonly available: false; readonly reason: typeof REASONS[number] }
export type ShellCapabilities = Readonly<Record<typeof CAPABILITIES[number], Capability>>

export function validCapabilities(value: unknown): value is ShellCapabilities {
  if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).length !== CAPABILITIES.length) return false
  return CAPABILITIES.every(key => {
    if (!Object.hasOwn(value, key)) return false
    const decision = (value as Record<string, unknown>)[key]
    if (!decision || typeof decision !== 'object' || Array.isArray(decision)) return false
    const d = decision as Record<string, unknown>
    return Object.keys(d).length === 2 && Object.hasOwn(d, 'available') && Object.hasOwn(d, 'reason')
      && (d.available === true ? d.reason === null : d.available === false && REASONS.some(reason => reason === d.reason))
  })
}
export function projectCapabilities(value: unknown): ShellCapabilities {
  if (!validCapabilities(value)) throw new Error('Invalid capabilities')
  return Object.freeze(Object.fromEntries(CAPABILITIES.map(key => [key, Object.freeze({ ...value[key] })]))) as ShellCapabilities
}
