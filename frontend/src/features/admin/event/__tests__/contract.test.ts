import { expect, it } from 'vitest'
import admin from '../../../../../../contracts/domain/v1/admin.schema.json'
import feed from '../../../../../../contracts/domain/v1/feed.schema.json'
import operations from '../../../../../../contracts/domain/v1/operations.json'
import { createNoticeSchema, noticeSchema, overviewSchema, setPhaseSchema, setSettingsSchema, settingsSchema, updateNoticeSchema } from '../model/contract'
import { adminPlacesSchema, placeAdminSchema, setPlaceSchema } from '../../places/contract'
import { auditListSchema } from '../../audit/contract'
import { noticeInput, settingsInput } from './fixtures/data'

it('pins consumer property/required sets to current C04 admin/place/notice definitions', () => {
  for (const [schema, contract] of [
    [settingsSchema, admin.$defs.EventSettings], [setSettingsSchema, admin.$defs.SetEventSettings], [setPhaseSchema, admin.$defs.SetEventPhase],
    [createNoticeSchema, admin.$defs.CreateNotice], [updateNoticeSchema, admin.$defs.PublishNotice], [noticeSchema, feed.$defs.Notice],
    [overviewSchema, admin.$defs.Overview], [placeAdminSchema, admin.$defs.PlaceAdmin], [setPlaceSchema, admin.$defs.SetPlace],
    [adminPlacesSchema, admin.$defs.PlaceAdminList], [auditListSchema, admin.$defs.AuditList],
  ] as const) {
    expect(Object.keys(schema.shape).sort()).toEqual([...contract.required].sort())
    expect(Object.keys(schema.shape).sort()).toEqual(Object.keys(contract.properties).sort())
  }
})
it('pins the selected operation methods, targets and response/pending status instead of inventing transports', () => {
  for (const [id, method, path] of [
    ['setConcertSettings', 'put', '/admin/settings'], ['setConcertEventPhase', 'post', '/admin/phase'], ['setConcertPlace', 'put', '/admin/places/{placeId}'],
    ['createConcertNotice', 'post', '/admin/notices'], ['updateConcertNotice', 'put', '/admin/notices/{noticeId}'],
  ]) {
    const op = operations.find(o => o.operationId === id)!
    expect(op).toMatchObject({ method, path: `/api/concert/events/{eventId}${path}`, success_status: 200, pending_status: 202, actor: 'OPERATOR', idempotency_key: true })
  }
  expect(admin.$defs.SetEventPhase.properties.phase.enum).toEqual(['OPEN', 'LIVE'])
})
it.each([
  ['beacon_ttl_seconds', 0, 901], ['pickup_grace_seconds', -1, 86401], ['social_grant_ttl_seconds', 0, 86401], ['mural_cooldown_ms', -1, 3600001],
] as const)('enforces %s finite bounds before review', (key, low, high) => {
  for (const value of [low, high, NaN, Infinity, 1.5]) expect(setSettingsSchema.safeParse({ ...settingsInput(), [key]: value }).success).toBe(false)
})
it('accepts the exact source-stamped notice contract and rejects added private media or arbitrary fields', () => {
  expect(createNoticeSchema.safeParse(noticeInput()).success).toBe(true)
  expect(createNoticeSchema.safeParse({ ...noticeInput(), media_uri: 'https://private.invalid/report' }).success).toBe(false)
})
