import type { LocalInputConfiguration } from './resolver'
const keys = (v: object, expected: string[]) => Object.keys(v).length === expected.length && expected.every(k => Object.hasOwn(v, k))
const record = (v: unknown): v is Record<string, unknown> => v !== null && typeof v === 'object' && !Array.isArray(v) && (Object.getPrototypeOf(v) === Object.prototype || Object.getPrototypeOf(v) === null) && Object.getOwnPropertySymbols(v).length === 0 && Object.values(Object.getOwnPropertyDescriptors(v)).every(d => Object.hasOwn(d, 'value'))
/** Deployment configuration only. Never infer movement constants from a fixture or server tick. */
export function validateLocalInputConfiguration(value: unknown): LocalInputConfiguration | null {
  if (!record(value) || !keys(value, ['revision', 'reviewedBy', 'movementPolicy', 'zoom'])) return null
  const { revision, reviewedBy, movementPolicy: p, zoom } = value
  if (typeof revision !== 'string' || !revision.trim() || revision.length > 128 || typeof reviewedBy !== 'string' || !reviewedBy.trim() || reviewedBy.length > 128
    || !record(p) || !keys(p, ['speedPxPerSecond', 'footWidthPx', 'footHeightPx', 'interactionRadiusPx'])
    || !Object.values(p).every(n => typeof n === 'number' && Number.isFinite(n)) || typeof zoom !== 'number' || !Number.isFinite(zoom) || zoom < 1 || zoom > 8) return null
  const speed = p.speedPxPerSecond as number, width = p.footWidthPx as number, height = p.footHeightPx as number, radius = p.interactionRadiusPx as number
  if (speed <= 0 || speed > 1024 || width < 1 || width > 128 || height < 1 || height > 128 || radius <= 0 || radius > 512) return null
  // The resolver additionally checks the foot bounds against the authorized map's tile dimensions.
  return Object.freeze({ revision, reviewedBy, movementPolicy: Object.freeze({ speedPxPerSecond: speed, footWidthPx: width, footHeightPx: height, interactionRadiusPx: radius }), zoom })
}
export function readLocalInputConfiguration(raw: unknown): LocalInputConfiguration | null {
  if (typeof raw !== 'string' || raw.length > 4096) return null
  try { return validateLocalInputConfiguration(JSON.parse(raw)) } catch { return null }
}
