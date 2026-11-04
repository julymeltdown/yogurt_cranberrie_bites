import original from './style-source.json'

function immutable<T>(value: T): T {
  if (value && typeof value === 'object') { Object.values(value).forEach(immutable); Object.freeze(value) }
  return value
}
/** Independent compact v2 paint values; original font/icon data remains exact and immutable. */
export const compositionStyle = immutable(structuredClone(original))
