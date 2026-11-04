/** Build metadata is a compatibility claim. It grants no identity or access. */
export interface ApplicationRelease {
  readonly version: 1
  readonly applicationVersion: string
  readonly buildId: string
  readonly sourceRevision: string
}

export function applicationVersionTuple(value: unknown): readonly [number, number, number] | null {
  if (typeof value !== 'string' || value.length > 17 || /[^0-9.]/.test(value)
    || !/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/.test(value)) return null
  const values = value.split('.').map(Number)
  if (values.length !== 3 || values.some(part => !Number.isInteger(part) || part > 65535)) return null
  return Object.freeze([values[0]!, values[1]!, values[2]!])
}

export function readApplicationRelease(value: unknown): ApplicationRelease | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  try {
    const descriptors = Object.getOwnPropertyDescriptors(value)
    if (Reflect.ownKeys(value).map(String).sort().join(',') !== 'applicationVersion,buildId,sourceRevision,version'
      || Object.values(descriptors).some(field => !Object.hasOwn(field, 'value') || !field.enumerable)) return null
    const version: unknown = descriptors.version?.value
    const applicationVersion: unknown = descriptors.applicationVersion?.value
    const buildId: unknown = descriptors.buildId?.value
    const sourceRevision: unknown = descriptors.sourceRevision?.value
    if (version !== 1 || !applicationVersionTuple(applicationVersion)
      || typeof buildId !== 'string' || buildId.length !== 64 || !/^[a-f0-9]{64}$/.test(buildId)
      || typeof sourceRevision !== 'string' || sourceRevision.length !== 40 || !/^[a-f0-9]{40}$/.test(sourceRevision)) return null
    return Object.freeze({ version: 1, applicationVersion: applicationVersion as string, buildId, sourceRevision })
  } catch { return null }
}

/** Capture once for each bootstrap attempt; missing injection remains unavailable. */
export function capturedApplicationRelease(): ApplicationRelease | null {
  return readApplicationRelease(import.meta.env?.VITE_MCR_APPLICATION_RELEASE)
}
