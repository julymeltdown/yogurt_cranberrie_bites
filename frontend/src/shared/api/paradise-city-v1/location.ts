/** Location1.0.1 wire leaf. Source7e516c3317f3caedbd3059be5ce4d13e5480e9ab.
 * API SHA394c52c485df8c63bfd7843932ea333b1f31cfd4aadbec705992fdbd9e06516b.
 * Root owns registration; these literals are NOT DomainOperationId assertions.
 */
import type { DecodeContext, Decoder, HttpRequest, HttpResult } from '../http'
import type { PrivacyScope } from '../../query/privacy'
import { TransportError } from '../errors'

export type LandmarkId = string

export type LandmarkSelection = { readonly landmark_id: LandmarkId; readonly catalog_revision: number; readonly landmark_revision: number }

export type PhysicalPlaceBinding = { readonly physical_place_id: string; readonly current_place_revision: number }

export type LandmarkGrant = { readonly event_id: string; readonly selection: LandmarkSelection; readonly provider_id: string; readonly source_revision: string; readonly venue_key: string; readonly map_id: string; readonly map_revision: number; readonly policy_revision: number; readonly label: string; readonly review_reference: string; readonly reviewed_at: string; readonly server_time: string; readonly valid_until: string; readonly physical_binding: PhysicalPlaceBinding | null; readonly presentation_id: string | null; readonly can_check_in: boolean; readonly can_meet: boolean; readonly can_note: boolean }

export type VenueClock = { readonly event_id: string; readonly venue_key: string; readonly policy_revision: number; readonly timezone: string; readonly server_time: string; readonly end_of_day: string; readonly valid_until: string }

export type VenueLocationPolicy = { readonly event_id: string; readonly venue_key: string; readonly provider_id: string; readonly source_revision: string; readonly revision: number; readonly timezone: string; readonly valid_until: string; readonly fresh_ms: number; readonly last_known_ms: number; readonly fix_max_age_ms: number; readonly future_skew_ms: number; readonly sharing_lease_ms: number; readonly request_ttl_ms: number; readonly check_in_ttl_ms: number; readonly coordinate_retention_ms: number; readonly command_retention_ms: number; readonly max_accuracy_m: number; readonly max_viewers: number }

export type PhoneFixInput = { readonly expected_sharing_revision: number; readonly source: "DEVICE_GEOLOCATION_REPORTED"; readonly latitude: number; readonly longitude: number; readonly accuracy_m: number; readonly captured_at: string }

export type PhonePosition = { readonly latitude: number; readonly longitude: number; readonly accuracy_m: number; readonly captured_at: string; readonly received_at: string }

export type SharingInput = { readonly expected_revision: number; readonly state: "ACTIVE" | "PAUSED" | "REVOKED"; readonly viewer_ids: readonly (string)[] }

export type SharingState = { readonly state: "OFF" | "ACTIVE" | "PAUSED" | "REVOKED" | "EXPIRED"; readonly revision: number; readonly viewer_ids: readonly (string)[]; readonly expires_at: string | null; readonly updated_at: string | null }

export type LocationCapabilities = { readonly can_share_phone: boolean; readonly can_pause: boolean; readonly can_revoke: boolean; readonly check_in_landmark_ids: readonly (LandmarkId)[]; readonly geographic_projection: "UNAVAILABLE" }

export type VisibleLocation = { readonly user_id: string; readonly source: "PHONE" | "SELF_REPORTED_CHECK_IN" | "LAST_KNOWN" | "UNAVAILABLE"; readonly position: PhonePosition | null; readonly landmark: LandmarkGrant | null; readonly observed_at: string | null; readonly fresh_until: string | null; readonly expires_at: string | null; readonly access_until: string; readonly visibility_revision: number; readonly physical_arrival_verified: false; readonly can_request_location: boolean }

export type LocationSnapshot = { readonly event_id: string; readonly actor_id: string; readonly server_time: string; readonly access_until: string; readonly policy: VenueLocationPolicy; readonly clock: VenueClock; readonly sharing: SharingState; readonly self_location: VisibleLocation | null; readonly friends: readonly (VisibleLocation)[]; readonly capabilities: LocationCapabilities; readonly incoming_requests: readonly (LocationRequest)[]; readonly outgoing_requests: readonly (LocationRequest)[] }

export type LocationRequestInput = { readonly subject_id: string }

export type LocationRequestDecisionInput = { readonly expected_revision: number; readonly decision: "APPROVE" | "DENY" | "CANCEL" }

export type CheckInInput = { readonly expected_sharing_revision: number; readonly landmark: LandmarkSelection }

export type LocationRequest = { readonly request_id: string; readonly event_id: string; readonly requester_id: string; readonly subject_id: string; readonly revision: number; readonly state: "PENDING" | "APPROVED" | "DENIED" | "CANCELLED" | "EXPIRED"; readonly created_at: string; readonly expires_at: string }

export type LocationCommandReceipt = { readonly command_key: string; readonly operation: "setVenueLocationSharing" | "publishVenueLocationFix" | "requestVenueLocation" | "decideVenueLocationRequest" | "setVenueLocationCheckIn"; readonly state: "SUCCEEDED" | "FAILED"; readonly resource_kind: "SHARING" | "PHONE_FIX" | "LOCATION_REQUEST" | "CHECK_IN"; readonly resource_id: string; readonly revision: number; readonly created_at: string; readonly expires_at: string }

export type LandmarkList = { readonly event_id: string; readonly server_time: string; readonly access_until: string; readonly items: readonly (LandmarkGrant)[] }

export type LocationError = { readonly code: string; readonly message: string; readonly traceId: string; readonly timestamp: string; readonly details: { readonly reason: "INVALID_INPUT" | "AUTHENTICATION_REQUIRED" | "RESOURCE_UNAVAILABLE" | "DEPENDENCY_UNAVAILABLE" | "VERSION_CONFLICT" | "IDEMPOTENCY_CONFLICT" | "IDEMPOTENCY_EXPIRED" | "LOCATION_SHARING_OFF" | "LOCATION_STALE" | "LOCATION_ACCURACY_UNAVAILABLE" | "LOCATION_REQUEST_EXPIRED" | "LOCATION_REQUEST_TERMINAL" | "RATE_LIMITED" } }

interface WireSchema {
  readonly $ref?: string
  readonly type?: string
  readonly oneOf?: readonly WireSchema[]
  readonly properties?: Readonly<Record<string, WireSchema>>
  readonly required?: readonly string[]
  readonly additionalProperties?: boolean
  readonly items?: WireSchema
  readonly maxItems?: number
  readonly uniqueItems?: boolean
  readonly enum?: readonly unknown[]
  readonly const?: unknown
  readonly minLength?: number
  readonly maxLength?: number
  readonly pattern?: string
  readonly format?: string
  readonly minimum?: number
  readonly maximum?: number
  readonly exclusiveMinimum?: number
}
// Constraint-only transcription of the qualified schemas; descriptions omitted.
const schemas: Readonly<Record<string, WireSchema>> = {"LandmarkId":{"type":"string","minLength":1,"maxLength":128,"pattern":"^(?!\\.{1,2}$)[A-Za-z0-9._~-]+$"},"LandmarkSelection":{"type":"object","additionalProperties":false,"properties":{"landmark_id":{"$ref":"#/components/schemas/LandmarkId"},"catalog_revision":{"type":"integer","minimum":1,"maximum":9007199254740991},"landmark_revision":{"type":"integer","minimum":1,"maximum":9007199254740991}},"required":["landmark_id","catalog_revision","landmark_revision"]},"PhysicalPlaceBinding":{"type":"object","additionalProperties":false,"properties":{"physical_place_id":{"type":"string","format":"uuid"},"current_place_revision":{"type":"integer","minimum":1,"maximum":9007199254740991}},"required":["physical_place_id","current_place_revision"]},"LandmarkGrant":{"type":"object","additionalProperties":false,"properties":{"event_id":{"type":"string","format":"uuid"},"selection":{"$ref":"#/components/schemas/LandmarkSelection"},"provider_id":{"type":"string","minLength":1,"maxLength":128},"source_revision":{"type":"string","minLength":1,"maxLength":128},"venue_key":{"type":"string","minLength":1,"maxLength":128},"map_id":{"type":"string","minLength":1,"maxLength":128},"map_revision":{"type":"integer","minimum":1,"maximum":9007199254740991},"policy_revision":{"type":"integer","minimum":1,"maximum":9007199254740991},"label":{"type":"string","minLength":1,"maxLength":160},"review_reference":{"type":"string","minLength":1,"maxLength":256},"reviewed_at":{"type":"string","format":"date-time"},"server_time":{"type":"string","format":"date-time"},"valid_until":{"type":"string","format":"date-time"},"physical_binding":{"oneOf":[{"$ref":"#/components/schemas/PhysicalPlaceBinding"},{"type":"null"}]},"presentation_id":{"oneOf":[{"type":"string","minLength":1,"maxLength":128},{"type":"null"}]},"can_check_in":{"type":"boolean"},"can_meet":{"type":"boolean"},"can_note":{"type":"boolean"}},"required":["event_id","selection","provider_id","source_revision","venue_key","map_id","map_revision","policy_revision","label","review_reference","reviewed_at","server_time","valid_until","physical_binding","presentation_id","can_check_in","can_meet","can_note"]},"VenueClock":{"type":"object","additionalProperties":false,"properties":{"event_id":{"type":"string","format":"uuid"},"venue_key":{"type":"string","minLength":1,"maxLength":128},"policy_revision":{"type":"integer","minimum":1,"maximum":9007199254740991},"timezone":{"type":"string","minLength":1,"maxLength":128},"server_time":{"type":"string","format":"date-time"},"end_of_day":{"type":"string","format":"date-time"},"valid_until":{"type":"string","format":"date-time"}},"required":["event_id","venue_key","policy_revision","timezone","server_time","end_of_day","valid_until"]},"VenueLocationPolicy":{"type":"object","additionalProperties":false,"properties":{"event_id":{"type":"string","format":"uuid"},"venue_key":{"type":"string","minLength":1,"maxLength":128},"provider_id":{"type":"string","minLength":1,"maxLength":128},"source_revision":{"type":"string","minLength":1,"maxLength":128},"revision":{"type":"integer","minimum":1,"maximum":9007199254740991},"timezone":{"type":"string","minLength":1,"maxLength":128},"valid_until":{"type":"string","format":"date-time"},"fresh_ms":{"type":"integer","minimum":1,"maximum":9007199254740991},"last_known_ms":{"type":"integer","minimum":1,"maximum":9007199254740991},"fix_max_age_ms":{"type":"integer","minimum":1,"maximum":9007199254740991},"future_skew_ms":{"type":"integer","minimum":0,"maximum":9007199254740991},"sharing_lease_ms":{"type":"integer","minimum":1,"maximum":9007199254740991},"request_ttl_ms":{"type":"integer","minimum":1,"maximum":9007199254740991},"check_in_ttl_ms":{"type":"integer","minimum":1,"maximum":9007199254740991},"coordinate_retention_ms":{"type":"integer","minimum":1,"maximum":9007199254740991},"command_retention_ms":{"type":"integer","minimum":1,"maximum":9007199254740991},"max_accuracy_m":{"type":"number","exclusiveMinimum":0},"max_viewers":{"type":"integer","minimum":1,"maximum":50}},"required":["event_id","venue_key","provider_id","source_revision","revision","timezone","valid_until","fresh_ms","last_known_ms","fix_max_age_ms","future_skew_ms","sharing_lease_ms","request_ttl_ms","check_in_ttl_ms","coordinate_retention_ms","command_retention_ms","max_accuracy_m","max_viewers"]},"PhoneFixInput":{"type":"object","additionalProperties":false,"properties":{"expected_sharing_revision":{"type":"integer","minimum":1,"maximum":9007199254740991},"source":{"type":"string","enum":["DEVICE_GEOLOCATION_REPORTED"]},"latitude":{"type":"number","minimum":-90,"maximum":90},"longitude":{"type":"number","minimum":-180,"maximum":180},"accuracy_m":{"type":"number","minimum":0},"captured_at":{"type":"string","format":"date-time"}},"required":["expected_sharing_revision","source","latitude","longitude","accuracy_m","captured_at"]},"PhonePosition":{"type":"object","additionalProperties":false,"properties":{"latitude":{"type":"number","minimum":-90,"maximum":90},"longitude":{"type":"number","minimum":-180,"maximum":180},"accuracy_m":{"type":"number","minimum":0},"captured_at":{"type":"string","format":"date-time"},"received_at":{"type":"string","format":"date-time"}},"required":["latitude","longitude","accuracy_m","captured_at","received_at"]},"SharingInput":{"type":"object","additionalProperties":false,"properties":{"expected_revision":{"type":"integer","minimum":0,"maximum":9007199254740991},"state":{"type":"string","enum":["ACTIVE","PAUSED","REVOKED"]},"viewer_ids":{"type":"array","items":{"type":"string","format":"uuid"},"maxItems":50,"uniqueItems":true}},"required":["expected_revision","state","viewer_ids"]},"SharingState":{"type":"object","additionalProperties":false,"properties":{"state":{"type":"string","enum":["OFF","ACTIVE","PAUSED","REVOKED","EXPIRED"]},"revision":{"type":"integer","minimum":0,"maximum":9007199254740991},"viewer_ids":{"type":"array","items":{"type":"string","format":"uuid"},"maxItems":50,"uniqueItems":true},"expires_at":{"oneOf":[{"type":"string","format":"date-time"},{"type":"null"}]},"updated_at":{"oneOf":[{"type":"string","format":"date-time"},{"type":"null"}]}},"required":["state","revision","viewer_ids","expires_at","updated_at"]},"LocationCapabilities":{"type":"object","additionalProperties":false,"properties":{"can_share_phone":{"type":"boolean"},"can_pause":{"type":"boolean"},"can_revoke":{"type":"boolean"},"check_in_landmark_ids":{"type":"array","items":{"$ref":"#/components/schemas/LandmarkId"},"maxItems":50},"geographic_projection":{"type":"string","enum":["UNAVAILABLE"]}},"required":["can_share_phone","can_pause","can_revoke","check_in_landmark_ids","geographic_projection"]},"VisibleLocation":{"type":"object","additionalProperties":false,"properties":{"user_id":{"type":"string","format":"uuid"},"source":{"type":"string","enum":["PHONE","SELF_REPORTED_CHECK_IN","LAST_KNOWN","UNAVAILABLE"]},"position":{"oneOf":[{"$ref":"#/components/schemas/PhonePosition"},{"type":"null"}]},"landmark":{"oneOf":[{"$ref":"#/components/schemas/LandmarkGrant"},{"type":"null"}]},"observed_at":{"oneOf":[{"type":"string","format":"date-time"},{"type":"null"}]},"fresh_until":{"oneOf":[{"type":"string","format":"date-time"},{"type":"null"}]},"expires_at":{"oneOf":[{"type":"string","format":"date-time"},{"type":"null"}]},"access_until":{"type":"string","format":"date-time"},"visibility_revision":{"type":"integer","minimum":1,"maximum":9007199254740991},"physical_arrival_verified":{"type":"boolean","const":false},"can_request_location":{"type":"boolean"}},"required":["user_id","source","position","landmark","observed_at","fresh_until","expires_at","access_until","visibility_revision","physical_arrival_verified","can_request_location"]},"LocationSnapshot":{"type":"object","additionalProperties":false,"properties":{"event_id":{"type":"string","format":"uuid"},"actor_id":{"type":"string","format":"uuid"},"server_time":{"type":"string","format":"date-time"},"access_until":{"type":"string","format":"date-time"},"policy":{"$ref":"#/components/schemas/VenueLocationPolicy"},"clock":{"$ref":"#/components/schemas/VenueClock"},"sharing":{"$ref":"#/components/schemas/SharingState"},"self_location":{"oneOf":[{"$ref":"#/components/schemas/VisibleLocation"},{"type":"null"}]},"friends":{"type":"array","items":{"$ref":"#/components/schemas/VisibleLocation"},"maxItems":50},"capabilities":{"$ref":"#/components/schemas/LocationCapabilities"},"incoming_requests":{"type":"array","items":{"$ref":"#/components/schemas/LocationRequest"},"maxItems":50},"outgoing_requests":{"type":"array","items":{"$ref":"#/components/schemas/LocationRequest"},"maxItems":50}},"required":["event_id","actor_id","server_time","access_until","policy","clock","sharing","self_location","friends","capabilities","incoming_requests","outgoing_requests"]},"LocationRequestInput":{"type":"object","additionalProperties":false,"properties":{"subject_id":{"type":"string","format":"uuid"}},"required":["subject_id"]},"LocationRequestDecisionInput":{"type":"object","additionalProperties":false,"properties":{"expected_revision":{"type":"integer","minimum":1,"maximum":9007199254740991},"decision":{"type":"string","enum":["APPROVE","DENY","CANCEL"]}},"required":["expected_revision","decision"]},"CheckInInput":{"type":"object","additionalProperties":false,"properties":{"expected_sharing_revision":{"type":"integer","minimum":1,"maximum":9007199254740991},"landmark":{"$ref":"#/components/schemas/LandmarkSelection"}},"required":["expected_sharing_revision","landmark"]},"LocationRequest":{"type":"object","additionalProperties":false,"properties":{"request_id":{"type":"string","format":"uuid"},"event_id":{"type":"string","format":"uuid"},"requester_id":{"type":"string","format":"uuid"},"subject_id":{"type":"string","format":"uuid"},"revision":{"type":"integer","minimum":1,"maximum":9007199254740991},"state":{"type":"string","enum":["PENDING","APPROVED","DENIED","CANCELLED","EXPIRED"]},"created_at":{"type":"string","format":"date-time"},"expires_at":{"type":"string","format":"date-time"}},"required":["request_id","event_id","requester_id","subject_id","revision","state","created_at","expires_at"]},"LocationCommandReceipt":{"type":"object","additionalProperties":false,"properties":{"command_key":{"type":"string","format":"uuid"},"operation":{"type":"string","enum":["setVenueLocationSharing","publishVenueLocationFix","requestVenueLocation","decideVenueLocationRequest","setVenueLocationCheckIn"]},"state":{"type":"string","enum":["SUCCEEDED","FAILED"]},"resource_kind":{"type":"string","enum":["SHARING","PHONE_FIX","LOCATION_REQUEST","CHECK_IN"]},"resource_id":{"type":"string","format":"uuid"},"revision":{"type":"integer","minimum":1,"maximum":9007199254740991},"created_at":{"type":"string","format":"date-time"},"expires_at":{"type":"string","format":"date-time"}},"required":["command_key","operation","state","resource_kind","resource_id","revision","created_at","expires_at"]},"LandmarkList":{"type":"object","additionalProperties":false,"properties":{"event_id":{"type":"string","format":"uuid"},"server_time":{"type":"string","format":"date-time"},"access_until":{"type":"string","format":"date-time"},"items":{"type":"array","items":{"$ref":"#/components/schemas/LandmarkGrant"},"maxItems":50}},"required":["event_id","server_time","access_until","items"]},"LocationError":{"type":"object","additionalProperties":false,"properties":{"code":{"type":"string","minLength":1,"maxLength":64},"message":{"type":"string","minLength":1,"maxLength":256},"traceId":{"type":"string","format":"uuid"},"timestamp":{"type":"string","format":"date-time"},"details":{"type":"object","additionalProperties":false,"properties":{"reason":{"type":"string","enum":["INVALID_INPUT","AUTHENTICATION_REQUIRED","RESOURCE_UNAVAILABLE","DEPENDENCY_UNAVAILABLE","VERSION_CONFLICT","IDEMPOTENCY_CONFLICT","IDEMPOTENCY_EXPIRED","LOCATION_SHARING_OFF","LOCATION_STALE","LOCATION_ACCURACY_UNAVAILABLE","LOCATION_REQUEST_EXPIRED","LOCATION_REQUEST_TERMINAL","RATE_LIMITED"]}},"required":["reason"]}},"required":["code","message","traceId","timestamp","details"]}}

const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/
const landmarkPattern = /^(?!\.{1,2}$)[A-Za-z0-9._~-]{1,128}$/
const invalid = (): never => { throw new TransportError('invalid-response') }
function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    && (Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null)
}
function time(value: string): number {
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d{1,9})?(Z|[+-](?:[01]\d|2[0-3]):[0-5]\d)$/.exec(value)
  if (!m) return invalid()
  const year = Number(m[1]), month = Number(m[2]), day = Number(m[3])
  const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0)
  const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
  if (month < 1 || month > 12 || day < 1 || day > days[month - 1]!
    || Number(m[4]) > 23 || Number(m[5]) > 59 || Number(m[6]) > 59) return invalid()
  const epoch = Date.parse(value)
  if (!Number.isFinite(epoch)) return invalid()
  return epoch
}
function validate(value: unknown, schema: WireSchema): void {
  if (schema.$ref) {
    const target = schemas[schema.$ref.replace('#/components/schemas/', '')]
    if (!target) return invalid()
    return validate(value, target)
  }
  if (schema.oneOf) {
    let count = 0
    for (const branch of schema.oneOf) {
      try { validate(value, branch); count++ } catch { /* exactly one branch must match */ }
    }
    if (count !== 1) return invalid()
    return
  }
  if (Object.hasOwn(schema, 'const') && value !== schema.const) return invalid()
  if (schema.enum && !schema.enum.includes(value)) return invalid()
  switch (schema.type) {
    case 'null': if (value !== null) return invalid(); break
    case 'boolean': if (typeof value !== 'boolean') return invalid(); break
    case 'integer':
    case 'number':
      if (typeof value !== 'number' || !Number.isFinite(value)
        || (schema.type === 'integer' && !Number.isSafeInteger(value))
        || (schema.minimum !== undefined && value < schema.minimum)
        || (schema.maximum !== undefined && value > schema.maximum)
        || (schema.exclusiveMinimum !== undefined && value <= schema.exclusiveMinimum)) return invalid()
      break
    case 'string':
      if (typeof value !== 'string' || (schema.minLength !== undefined && [...value].length < schema.minLength)
        || (schema.maxLength !== undefined && [...value].length > schema.maxLength)
        || (schema.pattern !== undefined && !new RegExp(schema.pattern).test(value))) return invalid()
      if (schema.format === 'uuid' && !uuidPattern.test(value)) return invalid()
      if (schema.format === 'date-time') time(value)
      break
    case 'array':
      if (!Array.isArray(value) || (schema.maxItems !== undefined && value.length > schema.maxItems)
        || !schema.items || Object.keys(value).length !== value.length
        || Reflect.ownKeys(value).length !== value.length + 1
        || Object.entries(Object.getOwnPropertyDescriptors(value)).some(([k,d]) => k !== 'length' && (!d.enumerable || !Object.hasOwn(d,'value')))) return invalid()
      for (const entry of value) validate(entry, schema.items)
      if (schema.uniqueItems && new Set(value.map(entry => JSON.stringify(entry))).size !== value.length) return invalid()
      break
    case 'object': {
      if (!record(value)) return invalid()
      const properties = schema.properties ?? {}
      const descriptors = Object.getOwnPropertyDescriptors(value)
      if (Reflect.ownKeys(value).length !== Object.keys(value).length
        || Object.values(descriptors).some(d => !d.enumerable || !Object.hasOwn(d, 'value'))
        || (schema.additionalProperties === false && Object.keys(value).some(k => !Object.hasOwn(properties, k)))
        || schema.required?.some(k => !Object.hasOwn(value, k))) return invalid()
      for (const [key, entry] of Object.entries(value)) {
        const child = properties[key]
        if (child) validate(entry, child)
      }
      break
    }
    default: return invalid()
  }
}
function frozenCopy(value: unknown): unknown {
  if (Array.isArray(value)) return Object.freeze(value.map(frozenCopy))
  if (record(value)) return Object.freeze(Object.fromEntries(Object.entries(value).map(([k, v]) => [k, frozenCopy(v)])))
  return value
}
interface WireTypes {
  LocationSnapshot: LocationSnapshot
  LocationCommandReceipt: LocationCommandReceipt
  LandmarkList: LandmarkList
  LandmarkGrant: LandmarkGrant
  LocationError: LocationError
  SharingInput: SharingInput
  PhoneFixInput: PhoneFixInput
  LocationRequestInput: LocationRequestInput
  LocationRequestDecisionInput: LocationRequestDecisionInput
  CheckInInput: CheckInInput
}
function wire<K extends keyof WireTypes>(name: K, value: unknown): WireTypes[K] {
  validate(value, schemas[name]!)
  // This assertion follows exhaustive frozen-schema validation, not an operation registry cast.
  return frozenCopy(value) as WireTypes[K]
}
function eventContext(eventId: string, context: DecodeContext): void {
  if (context.status !== 200 || context.scope.eventId !== eventId || !context.scope.userId) return invalid()
}
function landmarkContext(value: LandmarkGrant, eventId: string): void {
  if (value.event_id !== eventId || time(value.reviewed_at) > time(value.server_time)
    || time(value.valid_until) <= time(value.server_time)) return invalid()
}
function visibleContext(value: VisibleLocation, eventId: string): void {
  if ((value.source === 'PHONE' || value.source === 'LAST_KNOWN') && (!value.position || value.landmark)) return invalid()
  if (value.source === 'SELF_REPORTED_CHECK_IN' && (!value.landmark || value.position)) return invalid()
  if (value.source === 'UNAVAILABLE' && (value.position || value.landmark)) return invalid()
  if (value.landmark) landmarkContext(value.landmark, eventId)
}
export const decodeLocationSnapshot: Decoder<LocationSnapshot> = (value, context) => {
  const v = wire('LocationSnapshot', value)
  eventContext(v.event_id, context)
  if (v.actor_id !== context.scope.userId || v.policy.event_id !== v.event_id || v.clock.event_id !== v.event_id
    || v.clock.venue_key !== v.policy.venue_key || v.clock.policy_revision !== v.policy.revision
    || v.policy.fresh_ms > v.policy.last_known_ms || v.policy.last_known_ms > v.policy.coordinate_retention_ms
    || v.policy.fix_max_age_ms > v.policy.last_known_ms
    || time(v.access_until) <= time(v.server_time) || time(v.policy.valid_until) <= time(v.server_time)
    || time(v.clock.valid_until) <= time(v.server_time) || time(v.clock.end_of_day) <= time(v.clock.server_time)
    || (v.sharing.state !== 'ACTIVE' && v.sharing.viewer_ids.length !== 0)) return invalid()
  if (v.self_location) {
    if (v.self_location.user_id !== v.actor_id) return invalid()
    visibleContext(v.self_location, v.event_id)
  }
  const friends = new Set<string>()
  for (const friend of v.friends) {
    if (friend.user_id === v.actor_id || friends.has(friend.user_id)) return invalid()
    friends.add(friend.user_id); visibleContext(friend, v.event_id)
  }
  const requests = new Set<string>()
  for (const [direction, rows] of [['incoming', v.incoming_requests], ['outgoing', v.outgoing_requests]] as const) {
    let previous: LocationRequest | undefined
    for (const row of rows) {
      if (row.event_id !== v.event_id || requests.has(row.request_id)
        || (direction === 'incoming' ? row.subject_id !== v.actor_id || !friends.has(row.requester_id)
          : row.requester_id !== v.actor_id || !friends.has(row.subject_id))
        || time(row.expires_at) <= time(row.created_at)
        || (row.state === 'PENDING' && time(row.expires_at) <= time(v.server_time))
        || (previous && (time(previous.created_at) < time(row.created_at)
          || (time(previous.created_at) === time(row.created_at) && previous.request_id < row.request_id)))) return invalid()
      requests.add(row.request_id); previous = row
    }
  }
  return v
}
export const decodeLandmarkGrant: Decoder<LandmarkGrant> = (value, context) => {
  const v = wire('LandmarkGrant', value); eventContext(v.event_id, context); landmarkContext(v, v.event_id); return v
}
export const decodeLandmarkList: Decoder<LandmarkList> = (value, context) => {
  const v = wire('LandmarkList', value); eventContext(v.event_id, context)
  if (time(v.access_until) <= time(v.server_time)) return invalid()
  const ids = new Set<string>()
  for (const item of v.items) {
    landmarkContext(item, v.event_id)
    if (ids.has(item.selection.landmark_id)) return invalid()
    ids.add(item.selection.landmark_id)
  }
  return v
}
const receiptKinds = {
  setVenueLocationSharing: 'SHARING', publishVenueLocationFix: 'PHONE_FIX',
  requestVenueLocation: 'LOCATION_REQUEST', decideVenueLocationRequest: 'LOCATION_REQUEST',
  setVenueLocationCheckIn: 'CHECK_IN',
} as const
export type LocationWriteOperation = keyof typeof receiptKinds
export const decodeLocationCommandReceipt: Decoder<LocationCommandReceipt> = (value, context) => {
  const v = wire('LocationCommandReceipt', value)
  if (context.status !== 200 || !context.scope.userId || !context.scope.eventId
    || receiptKinds[v.operation] !== v.resource_kind || time(v.expires_at) <= time(v.created_at)) return invalid()
  return v
}
/** Wire error fixture boundary only; canonical transport retains HTTP/error ownership. */
export function decodeLocationError(value: unknown): LocationError { return wire('LocationError', value) }

export const locationOperations = Object.freeze({
  getVenueLocationSnapshot: ['GET', '/location'],
  setVenueLocationSharing: ['PUT', '/location/sharing'],
  publishVenueLocationFix: ['PUT', '/location/fix'],
  requestVenueLocation: ['POST', '/location/requests'],
  decideVenueLocationRequest: ['POST', '/location/requests/{requestId}/decision'],
  setVenueLocationCheckIn: ['PUT', '/location/check-in'],
  getVenueLocationCommand: ['GET', '/location/commands/{key}'],
  listVenueLandmarks: ['GET', '/landmarks'],
  getVenueLandmark: ['GET', '/landmarks/{landmarkId}'],
} as const)
export type LocationOperationId = keyof typeof locationOperations
export interface LocationRequestDescriptor<T> {
  readonly operationId: LocationOperationId
  readonly method: 'GET' | 'PUT' | 'POST'
  readonly path: string
  readonly access: 'protected'
  readonly errorFamily: 'domain'
  readonly cache: 'no-store'
  readonly query?: HttpRequest<T>['query']
  readonly body?: { readonly json: SharingInput | PhoneFixInput | LocationRequestInput | LocationRequestDecisionInput | CheckInInput }
  readonly idempotencyKey?: string
  readonly decode: Decoder<T>
}
export interface LocationCanonicalTransport {
  request<T>(descriptor: LocationRequestDescriptor<T>, signal: AbortSignal): Promise<HttpResult<T>>
}
export class LocationUnavailableError extends Error {
  readonly kind = 'UNAVAILABLE'
  constructor() { super('Location transport is unavailable.'); this.name = 'LocationUnavailableError' }
}
export interface LocationLease {
  readonly scope: PrivacyScope
  readonly signal: AbortSignal
  readonly isCurrent: (scope: PrivacyScope) => boolean
  /** Actual authoritative clock supplied by the owner. No Date.now fallback/calibration. */
  readonly serverNow: () => number
}
function requestId(value: string): string {
  if (typeof value !== 'string' || !uuidPattern.test(value)) throw new TransportError('invalid-request')
  return value
}
function input<K extends keyof WireTypes>(name: K, value: unknown): WireTypes[K] {
  try { return wire(name, value) } catch { throw new TransportError('invalid-request') }
}
const issued = new WeakSet<object>()
function descriptor<T>(operationId: LocationOperationId, eventId: string, tail: string,
  decode: Decoder<T>, extra: Pick<LocationRequestDescriptor<T>, 'body' | 'idempotencyKey' | 'query'> = {}): LocationRequestDescriptor<T> {
  requestId(eventId)
  const path = `/api/concert/events/${eventId}/venue${tail}`
  const guarded: Decoder<T> = (value, context) => {
    if (context.path !== path || context.scope.eventId !== eventId || context.status !== 200) return invalid()
    return decode(value, context)
  }
  const result = Object.freeze({ operationId, method: locationOperations[operationId][0], path,
    access: 'protected', errorFamily: 'domain', cache: 'no-store', decode: guarded, ...extra } as const)
  issued.add(result); return result
}
function receipt(operation: LocationWriteOperation, key: string): Decoder<LocationCommandReceipt> {
  return (value, context) => {
    const result = decodeLocationCommandReceipt(value, context)
    if (result.operation !== operation || result.command_key !== key) return invalid()
    return result
  }
}
export const locationRequests = Object.freeze({
  snapshot: (eventId: string) => descriptor('getVenueLocationSnapshot', eventId, '/location', decodeLocationSnapshot),
  landmarks: (eventId: string) => descriptor('listVenueLandmarks', eventId, '/landmarks', decodeLandmarkList),
  landmark(eventId: string, landmarkId: string) {
    if (typeof landmarkId !== 'string' || !landmarkPattern.test(landmarkId)) throw new TransportError('invalid-request')
    return descriptor('getVenueLandmark', eventId, `/landmarks/${landmarkId}`, (v, c) => {
      const grant = decodeLandmarkGrant(v, c)
      if (grant.selection.landmark_id !== landmarkId) return invalid()
      return grant
    })
  },
  setSharing(eventId: string, body: SharingInput, key: string) {
    const json = input('SharingInput', body); requestId(key)
    if (json.state !== 'ACTIVE' && json.viewer_ids.length) throw new TransportError('invalid-request')
    return descriptor('setVenueLocationSharing', eventId, '/location/sharing', receipt('setVenueLocationSharing', key),
      { body: Object.freeze({ json }), idempotencyKey: key })
  },
  publishFix(eventId: string, body: PhoneFixInput, key: string) {
    const json = input('PhoneFixInput', body); requestId(key)
    return descriptor('publishVenueLocationFix', eventId, '/location/fix', receipt('publishVenueLocationFix', key),
      { body: Object.freeze({ json }), idempotencyKey: key })
  },
  requestLocation(eventId: string, body: LocationRequestInput, key: string) {
    const json = input('LocationRequestInput', body); requestId(key)
    return descriptor('requestVenueLocation', eventId, '/location/requests', receipt('requestVenueLocation', key),
      { body: Object.freeze({ json }), idempotencyKey: key })
  },
  decideRequest(eventId: string, request: string, body: LocationRequestDecisionInput, key: string) {
    const json = input('LocationRequestDecisionInput', body); requestId(key); requestId(request)
    return descriptor('decideVenueLocationRequest', eventId, `/location/requests/${request}/decision`, receipt('decideVenueLocationRequest', key),
      { body: Object.freeze({ json }), idempotencyKey: key })
  },
  checkIn(eventId: string, body: CheckInInput, key: string) {
    const json = input('CheckInInput', body); requestId(key)
    return descriptor('setVenueLocationCheckIn', eventId, '/location/check-in', receipt('setVenueLocationCheckIn', key),
      { body: Object.freeze({ json }), idempotencyKey: key })
  },
  command(eventId: string, operation: LocationWriteOperation, key: string) {
    requestId(key)
    if (typeof operation !== 'string' || !Object.hasOwn(receiptKinds, operation)) throw new TransportError('invalid-request')
    return descriptor('getVenueLocationCommand', eventId, `/location/commands/${key}`, receipt(operation, key),
      { query: Object.freeze({ operation }) })
  },
})
function sameScope(a: unknown, b: PrivacyScope): boolean {
  return record(a) && a.generation === b.generation && a.userId === b.userId && a.eventId === b.eventId
}
export function createLocationClient(transport: LocationCanonicalTransport | null, lease: LocationLease) {
  const scope = Object.freeze({ generation: lease.scope.generation, userId: lease.scope.userId,
    eventId: lease.scope.eventId, policyGeneration: lease.scope.policyGeneration, gate: lease.scope.gate })
  const assertCurrent = (uncertain = false) => {
    if (scope.gate !== 'open' || !scope.userId || !scope.eventId || !uuidPattern.test(scope.userId) || !uuidPattern.test(scope.eventId)
      || !Number.isSafeInteger(scope.generation) || scope.generation < 0 || !Number.isSafeInteger(scope.policyGeneration)
      || scope.policyGeneration < 0 || !lease.isCurrent(scope)) throw new TransportError('session-changed', { uncertain })
    if (lease.signal.aborted) throw new TransportError('aborted', { uncertain })
    const now = lease.serverNow()
    if (!Number.isFinite(now) || now < 0) throw new TransportError('invalid-request')
    return now
  }
  function currentData(value: unknown, now: number): void {
    if (!record(value)) return invalid()
    if (Object.hasOwn(value, 'access_until') && (typeof value.access_until !== 'string' || time(value.access_until) <= now)) return invalid()
    if (Object.hasOwn(value, 'valid_until') && (typeof value.valid_until !== 'string' || time(value.valid_until) <= now)) return invalid()
    if (Array.isArray(value.friends)) {
      for (const item of value.friends) currentData(item, now)
      if (value.self_location !== null) currentData(value.self_location, now)
      currentData(value.policy, now); currentData(value.clock, now)
      for (const rows of [value.incoming_requests, value.outgoing_requests]) {
        if (!Array.isArray(rows)) return invalid()
        for (const row of rows) if (record(row) && row.state === 'PENDING' && (typeof row.expires_at !== 'string' || time(row.expires_at) <= now)) return invalid()
      }
    }
    if (record(value.landmark)) currentData(value.landmark, now)
    if (Array.isArray(value.items)) for (const item of value.items) currentData(item, now)
    if (Object.hasOwn(value, 'expires_at') && Object.hasOwn(value, 'command_key')
      && (typeof value.expires_at !== 'string' || time(value.expires_at) <= now)) return invalid()
  }
  async function execute<T>(request: LocationRequestDescriptor<T>): Promise<HttpResult<T>> {
    assertCurrent()
    if (!transport) throw new LocationUnavailableError()
    const assertDispatched = () => assertCurrent(request.method !== 'GET')
    if (!issued.has(request)) throw new TransportError('invalid-request')
    const guarded = Object.freeze({ ...request, decode: ((value, context) => {
      const now = assertDispatched()
      if (!sameScope(context.scope, scope)) return invalid()
      const data = request.decode(value, context); currentData(data, now); currentData(data, assertDispatched()); return data
    }) satisfies Decoder<T> })
    let abort: () => void = () => undefined
    const stopped = new Promise<never>((_, reject) => {
      abort = () => reject(new TransportError('aborted', { uncertain: request.method !== 'GET' }))
      lease.signal.addEventListener('abort', abort, { once: true })
      if (lease.signal.aborted) abort()
    })
    try {
      // One dispatch; injected canonical transport owns auth/error normalization.
      const result = await Promise.race([transport.request(guarded, lease.signal), stopped])
      const now = assertDispatched()
      if (!sameScope(result.scope, scope)) return invalid()
      // Enforce decode even if an injected port returns data without calling it.
      const data = guarded.decode(result.data, { scope: result.scope, status: 200, path: request.path })
      currentData(data, now); currentData(data, assertDispatched())
      return Object.freeze({ data, scope: Object.freeze({ generation: result.scope.generation, userId: result.scope.userId, eventId: result.scope.eventId }) })
    } catch (error) {
      assertDispatched(); throw error
    } finally { lease.signal.removeEventListener('abort', abort) }
  }
  const event = () => { assertCurrent(); return scope.eventId! }
  return Object.freeze({
    getSnapshot: () => execute(locationRequests.snapshot(event())),
    listLandmarks: () => execute(locationRequests.landmarks(event())),
    getLandmark: (id: string) => execute(locationRequests.landmark(event(), id)),
    setSharing: (body: SharingInput, key: string) => execute(locationRequests.setSharing(event(), body, key)),
    publishFix: (body: PhoneFixInput, key: string) => execute(locationRequests.publishFix(event(), body, key)),
    requestLocation: (body: LocationRequestInput, key: string) => execute(locationRequests.requestLocation(event(), body, key)),
    decideRequest: (id: string, body: LocationRequestDecisionInput, key: string) => execute(locationRequests.decideRequest(event(), id, body, key)),
    checkIn: (body: CheckInInput, key: string) => execute(locationRequests.checkIn(event(), body, key)),
    getCommand: (operation: LocationWriteOperation, key: string) => execute(locationRequests.command(event(), operation, key)),
  })
}
