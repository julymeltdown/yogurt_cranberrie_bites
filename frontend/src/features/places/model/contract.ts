import { z } from 'zod'
import type { Decoder } from '../../../shared/api/http'
import contract from '../../../../../contracts/generated/mcr-core.openapi.json'

const text = z.string().min(1).max(512)
export const placeId = z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)
const provenance = z.strictObject({
  source_id: z.string().regex(/^[a-z][a-z0-9_]{0,63}$/),
  source_kind: z.enum(['ORIGINAL_FIXTURE', 'ORIGINAL_ART', 'PLACE_REFERENCE']),
  source_revision: text, source_uri: z.url().nullable(), author: text, license: text,
  review_state: z.enum(['UNREVIEWED', 'REVIEWED', 'REJECTED']),
  reviewed_by: text.nullable(), reviewed_at: z.iso.datetime({ offset: true }).nullable(),
  photo_permission: z.enum(['NOT_APPLICABLE', 'UNREVIEWED', 'GRANTED', 'DENIED']),
}).refine(p => p.review_state === 'REVIEWED' ? p.reviewed_by !== null && p.reviewed_at !== null
  : p.reviewed_by === null && p.reviewed_at === null)
export const placeSchema = z.strictObject({
  place_id: placeId, label: text, place_kind: z.enum(['VIRTUAL_CONCEPT', 'PHYSICAL']),
  verification_state: z.enum(['CONCEPT_ONLY', 'UNVERIFIED', 'VERIFIED']),
  closure_state: z.enum(['OPEN', 'CLOSED']), accessibility_notes: text, provenance,
}).refine(p => (p.place_kind !== 'VIRTUAL_CONCEPT' || p.verification_state === 'CONCEPT_ONLY')
  && (p.place_kind !== 'PHYSICAL' || (p.verification_state !== 'CONCEPT_ONLY'
    && p.provenance.source_kind === 'PLACE_REFERENCE' && p.provenance.source_uri !== null))
  && (p.verification_state !== 'VERIFIED' || (p.place_kind === 'PHYSICAL' && p.provenance.review_state === 'REVIEWED'))
  && (p.provenance.source_kind !== 'ORIGINAL_FIXTURE' || p.place_kind === 'VIRTUAL_CONCEPT'))
const revision = z.number().int().min(1).max(Number.MAX_SAFE_INTEGER)
const listSchema = z.strictObject({ event_id: placeId, revision, items: z.array(placeSchema).max(128) })
  .refine(v => new Set(v.items.map(p => p.place_id)).size === v.items.length)
const detailSchema = z.strictObject({ event_id: placeId, revision, place: placeSchema })
export type Place = z.infer<typeof placeSchema>
export type PlaceList = z.infer<typeof listSchema>
export type PlaceResponse = z.infer<typeof detailSchema>
// The generated C03 operations remain authoritative. This is a feature-local consumer,
// not a second endpoint registry or a broadened shared contract API.
const listOperation = contract.paths['/api/concert/events/{eventId}/places'].get
const detailOperation = contract.paths['/api/concert/events/{eventId}/places/{placeId}'].get
if (listOperation.operationId !== 'listConcertPlaces' || detailOperation.operationId !== 'getConcertPlace'
  || listOperation.responses[200].content['application/json'].schema.$ref !== '#/components/schemas/PlaceList'
  || detailOperation.responses[200].content['application/json'].schema.$ref !== '#/components/schemas/PlaceResponse') {
  throw new Error('Place contract changed')
}
export const decodePlaces: Decoder<PlaceList> = (value, context) => {
  const data = listSchema.parse(value)
  if (context.status !== 200 || data.event_id !== context.scope.eventId) throw new Error('Invalid place binding')
  return data
}
export const decodePlace: Decoder<PlaceResponse> = (value, context) => {
  const data = detailSchema.parse(value)
  if (context.status !== 200 || data.event_id !== context.scope.eventId
    || data.place.place_id !== context.path.split('/').at(-1)) throw new Error('Invalid place binding')
  return data
}
/** Source URIs can be valid contract URIs without being safe browser destinations. */
export function sourceLink(value: string | null): string | null {
  if (!value || value.length > 2048 || /[\s\\]/u.test(value)
    || Array.from(value).some(char => char.codePointAt(0)! < 32 || char.codePointAt(0) === 127)) return null
  try {
    const url = new URL(value)
    return url.protocol === 'https:' && !url.username && !url.password && Boolean(url.hostname) ? url.href : null
  } catch { return null }
}
export function physicalStatus(place: Place) {
  if (place.closure_state === 'CLOSED') return 'Closed — do not use as a meeting destination'
  if (place.place_kind === 'VIRTUAL_CONCEPT') return 'Virtual concept — no physical destination'
  if (place.verification_state !== 'VERIFIED' || place.provenance.review_state !== 'REVIEWED') return 'Physical place — verification pending'
  if (!sourceLink(place.provenance.source_uri)) return 'Physical reference — source unavailable'
  return 'Verified physical reference — check current venue guidance'
}
