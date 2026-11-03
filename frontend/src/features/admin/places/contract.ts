import { z } from 'zod'
import { placeSchema, type Place } from '../../places/model/contract'
import { cursor, revision, text } from '../event/model/contract'
export const placeAdminSchema = z.strictObject({ place: placeSchema, revision })
export const setPlaceSchema = z.strictObject({ expected_revision: revision, place: placeSchema, reason: text(500) })
export const adminPlacesSchema = z.strictObject({ items: z.array(placeAdminSchema).max(100), next_cursor: cursor })
  .refine(s => new Set(s.items.map(p => p.place.place_id)).size === s.items.length)
export type PlaceAdmin = z.infer<typeof placeAdminSchema>
export type SetPlace = z.infer<typeof setPlaceSchema>
export type { Place }
