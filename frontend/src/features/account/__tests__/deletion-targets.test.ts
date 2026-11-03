// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import domain from '../../../../../contracts/generated/mcr-domain.openapi.json'
import core from '../../../../../contracts/generated/mcr-core.openapi.json'
import { decodeDeletion, deletionTargets } from '../contract'
import { createAccountController, type AccountDeletionPort } from '../controller'
import { harness } from '../../safety/__tests__/fixtures'
import { deletionAccepted, deletionIds as ids, deletionNow as now, deletionSaga, requiredTargets } from './deletion-fixtures'

const binding = () => ({ actorId: ids.actor, sagaId: ids.saga, updatedAt: null as number | null })
const decode = (saga = deletionSaga(true)) => decodeDeletion(saga, 200, binding(), now)
const owned: { dispose(): void }[] = []
afterEach(() => owned.splice(0).forEach(h => h.dispose()))

it('matches both published canonical generated target inventories in order', () => {
  expect(deletionTargets).toEqual(requiredTargets)
  for (const schema of [domain, core]) expect(schema.components.schemas.UserDeletionStep.properties.target.enum).toEqual(requiredTargets)
})
it('accepts all eight completed targets and orders safe progress independently of wire order', () => {
  const saga = deletionSaga(true); saga.steps.reverse()
  const result = decode(saga)
  expect(result.progress.state).toBe('complete')
  expect(result.progress.steps.map(s => s.target)).toEqual(requiredTargets)
  expect(Object.isFrozen(result.progress.steps)).toBe(true)
  expect(result.binding).toEqual({ actorId: ids.actor, sagaId: ids.saga, updatedAt: Date.parse(saga.updatedAt) })
})
it.each(requiredTargets)('cannot complete with missing %s', target => {
  const saga = deletionSaga(true); saga.steps = saga.steps.filter(s => s.target !== target)
  expect(() => decode(saga)).toThrow()
})
it.each(requiredTargets)('cannot complete when %s has not succeeded', target => {
  for (const status of ['PENDING', 'IN_PROGRESS', 'RETRY', 'DEAD_LETTERED']) {
    const saga = deletionSaga(true); saga.steps.find(s => s.target === target)!.status = status
    expect(() => decode(saga)).toThrow()
  }
})
it.each(requiredTargets)('cannot complete without the %s completion timestamp', target => {
  const saga = deletionSaga(true); saga.steps.find(s => s.target === target)!.completedAt = null
  expect(() => decode(saga)).toThrow()
})
it.each(['legacy-four', 'duplicate', 'unknown', 'extra'] as const)('rejects %s inventory', kind => {
  const saga = deletionSaga(true)
  if (kind === 'legacy-four') saga.steps = saga.steps.slice(0, 4)
  if (kind === 'duplicate') saga.steps[7]!.target = 'PROFILE'
  if (kind === 'unknown') saga.steps[7]!.target = 'CONCERT_UNKNOWN'
  if (kind === 'extra') saga.steps.push({ ...saga.steps[0]!, target: 'CONCERT_UNKNOWN' })
  expect(() => decode(saga)).toThrow()
})
it.each(['IN_PROGRESS', 'RETRY', 'DEAD_LETTERED'])('keeps Concert %s visible and lastError private', status => {
  const saga = deletionSaga()
  for (const step of saga.steps.slice(0, 4)) { step.status = 'SUCCEEDED'; step.completedAt = '2026-11-04T05:30:00Z' }
  saga.steps[7]!.status = status; saga.steps[7]!.lastError = 'PRIVATE provider diagnostic'
  const result = decodeDeletion(deletionAccepted(saga), 202, binding(), now)
  expect(result.progress.state).toBe(status === 'DEAD_LETTERED' ? 'partial-failure' : 'pending')
  expect(result.progress.steps[7]).toEqual({ target: 'CONCERT_PRIVATE_DATA', state: status })
  expect(JSON.stringify(result.progress)).not.toContain('PRIVATE provider')
  expect(result.progress.steps).toHaveLength(8)
})
it.each(['future-update', 'backward-update', 'created-after-update', 'future-step', 'early-step', 'step-after-saga', 'missing-saga-time', 'future-saga-time', 'invalid-time'] as const)('rejects inconsistent completion metadata: %s', kind => {
  const saga = deletionSaga(true), bound = binding()
  if (kind === 'future-update') saga.updatedAt = '2026-11-04T06:01:00Z'
  if (kind === 'backward-update') bound.updatedAt = now
  if (kind === 'created-after-update') saga.createdAt = '2026-11-04T06:00:00Z'
  if (kind === 'future-step') saga.steps[7]!.completedAt = '2026-11-04T06:01:00Z'
  if (kind === 'early-step') saga.steps[7]!.completedAt = '2026-11-04T04:59:00Z'
  if (kind === 'step-after-saga') saga.steps[7]!.completedAt = '2026-11-04T05:58:30Z'
  if (kind === 'missing-saga-time') saga.completedAt = null
  if (kind === 'future-saga-time') saga.completedAt = '2026-11-04T06:01:00Z'
  if (kind === 'invalid-time') saga.steps[7]!.completedAt = 'invalid'
  expect(() => decodeDeletion(saga, 200, bound, now)).toThrow()
})
it.each(['actor', 'saga', 'envelope-actor', 'envelope-saga', 'not-disabled'] as const)('rejects mismatched %s', kind => {
  const value = deletionAccepted(deletionSaga(true)), bound = binding()
  if (kind === 'actor') value.saga.userId = ids.other
  if (kind === 'saga') bound.sagaId = ids.other
  if (kind === 'envelope-actor') value.userId = ids.other
  if (kind === 'envelope-saga') value.sagaId = ids.other
  if (kind === 'not-disabled') value.deleted = false
  expect(() => decodeDeletion(value, 202, bound, now)).toThrow()
})
it('legacy completed acknowledgment closes private owners and recovers only via explicit GET of all eight', async () => {
  const h = harness(() => new Response(null, { status: 204 })), done = vi.fn()
  const legacy = deletionSaga(true); legacy.steps = legacy.steps.slice(0, 4)
  const request = vi.fn<AccountDeletionPort['request']>(async scope => ({ scope, status: 202, data: deletionAccepted(legacy) }))
  const status = vi.fn<AccountDeletionPort['status']>(async scope => ({ scope, status: 200, data: deletionSaga(true) }))
  const account = createAccountController({ auth: h.auth, deletion: { request, status }, now: () => now, onDeletionComplete: done })
  owned.push({ dispose() { account.dispose(); h.dispose() } })
  h.auth.queryClient.setQueryData(['private', 'legacy'], 'PRIVATE')
  account.editConfirmation('DELETE_MY_ACCOUNT'); await account.requestDeletion()
  expect(account.read()).toMatchObject({ phase: 'unknown', progress: null })
  expect(h.auth.privacy.capture().gate).toBe('suspended')
  expect(h.auth.queryClient.getQueryData(['private', 'legacy'])).toBeUndefined()
  expect(done).not.toHaveBeenCalled(); expect(status).not.toHaveBeenCalled()
  await account.requestDeletion(); expect(request).toHaveBeenCalledTimes(1)
  await account.checkDeletion()
  expect(status).toHaveBeenCalledTimes(1); expect(status.mock.calls[0]![0].gate).toBe('suspended')
  expect(account.read().phase).toBe('complete'); expect(done).toHaveBeenCalledTimes(1)
  expect(h.auth.session.read().userId).toBeNull()
})
