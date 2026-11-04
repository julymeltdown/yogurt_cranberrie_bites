import original from '../map4-service-v1/style-source.json'

function immutable<T>(value: T): T {
  if (value && typeof value === 'object') { Object.values(value).forEach(immutable); Object.freeze(value) }
  return value
}
/** Exact retained style/font/icon values, with this presentation's own immutable copy. */
export const compositionStyle = immutable(structuredClone(original))
