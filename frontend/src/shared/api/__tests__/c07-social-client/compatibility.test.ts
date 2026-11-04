import { expect, it } from 'vitest'
import { domainOperation, matchesDomainRequest } from '../../domain'
import type { DomainOperationId } from '../../domain-operations.generated'
import { decodeContract } from '../../../../features/auth/model/contract'
import { introSchema } from '../../../../features/people/contract'
import { parse } from '../../../../features/event-feed/contract'
import { platform, requestState, socialRequest } from '../../../../features/social-sharing/model/contract'
import { assertOperations, inputManifest, operationIds, readJSON } from '../../../../../e2e/real/social/cohort.mjs'

const metadata = (id: string) => domainOperation(id as DomainOperationId)
it('matches the selected public operations, wire responses and retained request metadata', () => {
  assertOperations(metadata)
  for (const id of operationIds) {
    const op = metadata(id)!
    const path = op.path.replace(/\{[^}]+\}/g, '00000000-0000-4000-8000-000000000001')
    // Required query parameters are checked by their own retained metadata; no new route authority.
    expect(matchesDomainRequest(op, 'DELETE-DRIFT', path)).toBe(false)
  }
})
it('deliberate method, response and status drift fail with the affected operation', () => {
  const operations = readJSON('contracts/domain/v1/operations.json')
  for (const patch of [{ method: 'delete' }, { response: 'social.Unknown' }, { success_status: 201 }]) {
    const altered = structuredClone(operations)
    Object.assign(altered.find((o: { operationId: string }) => o.operationId === 'createConcertSocialRequest')!, patch)
    expect(() => assertOperations(metadata, altered)).toThrow(/C07 metadata drift createConcertSocialRequest/)
  }
})
it('public platform and request-state enums match the actual Zod consumers, including unknown-value rejection', () => {
  const schemas = readJSON('contracts/generated/mcr-domain.openapi.json').components.schemas
  expect(platform.options).toEqual(schemas.Domain_common_Platform.enum)
  expect(requestState.options).toEqual(schemas.Domain_social_SocialRequest.properties.state.enum)
  for (const patch of [{ state: 'NEW_STATE' }, { approved_platforms: ['NEW_PLATFORM'] }, { private_age: 25 }]) {
    expect(socialRequest.safeParse({ ...readJSON('contracts/fixtures/domain/approved-social-request.json'), ...patch }).success).toBe(false)
  }
  const changed = structuredClone(schemas.Domain_common_Platform.enum); changed.push('NEW_PLATFORM')
  expect(() => expect(platform.options).toEqual(changed)).toThrow()
})

const consumers: Record<string, (value: unknown) => unknown> = {
  'social.IntroPublic': value => introSchema.parse(value),
  'social.SocialRequest': value => socialRequest.parse(value),
  'social.SocialGrant': value => parse('Domain_social_SocialGrant', value),
  'feed.PostPublic': value => parse('Domain_feed_PostPublic', value),
  'feed.Notification': value => parse('Domain_feed_Notification', value),
  'feed.NotificationUnreadCount': value => parse('Domain_feed_NotificationUnreadCount', value),
}
const selected = readJSON('contracts/fixtures/domain/index.json').filter((row: { schema: string }) => Object.hasOwn(consumers, row.schema)) as { file: string; schema: string; expected: string }[]
it.each(selected)('indexed public fixture $file is $expected through the retained consumer', row => {
  const decode = () => consumers[row.schema]!(readJSON(`contracts/fixtures/domain/${row.file}`))
  if (row.expected === 'accept') expect(decode).not.toThrow()
  else expect(decode).toThrow()
})
it.each(['Bootstrap', 'AuthCapabilities', 'AuthResponse'] as const)('retained core %s decoder accepts its indexed public examples', name => {
  const rows = readJSON('contracts/fixtures/core/index.json').filter((row: { schema: string }) => row.schema === name) as { file: string }[]
  expect(rows.length).toBeGreaterThan(0)
  for (const row of rows) expect(() => decodeContract(name, readJSON(`contracts/fixtures/core/${row.file}`))).not.toThrow()
})
it('pins the exact public cohort without claiming all indexed fixtures are qualified', () => {
  const expected = readJSON('docs/mcr/evidence/c07-social-client-in03/inputs.json')
  expect(inputManifest()).toEqual(expected)
})
