import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import source from '../../../../../../contracts/fixtures/domain/public-intro.json'
import { ageBoundsSchema, matchesDiscoveryAge, matchesKnownAge, matchesSharingAge } from '../age'
import { ageStatusSchema, privateIntroSchema, publicIntroSchema, setIntroSchema } from '../contract'
import { introWriteReasonSchema } from '../errors'
import { introPolicyValid } from '../policy'
import { introContactBypass } from '../publication-contact'

const unbounded = { minimum_age: null, maximum_age_exclusive: null }
const own = () => ({ intro_id: source.intro_id, event_id: source.event_id, owner_id: source.author.user_id, text: '', age_bounds: unbounded, expires_at: null, created_at: source.created_at, state: 'DRAFT', revision: 1 })
const context = { now: Date.parse(source.created_at), eventCutoff: Date.parse(source.created_at) + 86_400_000, publishing: false }
describe('C04 form and projection boundaries', () => {
  it.each([{ minimum_age: 20 }, { maximum_age_exclusive: 30 }, {}, { minimum_age: null, maximum_age_exclusive: 30 }, { minimum_age: 20, maximum_age_exclusive: null }, { minimum_age: 20, maximum_age_exclusive: 20 }, { minimum_age: 30, maximum_age_exclusive: 20 }, { minimum_age: 13, maximum_age_exclusive: 20 }, { minimum_age: 100, maximum_age_exclusive: 102 }, { minimum_age: 14.5, maximum_age_exclusive: 20 }])('rejects incomplete/invalid bounds %j', value => expect(ageBoundsSchema.safeParse(value).success).toBe(false))
  it('retains half-open endpoints and explicit unknown semantics without asserting verification', () => {
    expect(ageBoundsSchema.parse(unbounded)).toEqual(unbounded)
    const range = { minimum_age: 14, maximum_age_exclusive: 101 }
    for (const n of [14, 100]) expect(matchesKnownAge(range, n)).toBe(true)
    for (const n of [13, 101, 20.5, NaN]) expect(matchesKnownAge(range, n)).toBe(false)
    expect(matchesKnownAge({ minimum_age: 20, maximum_age_exclusive: 30 }, 30)).toBe(false)
    expect(matchesDiscoveryAge(unbounded, null)).toBe(true)
    expect(matchesDiscoveryAge(range, null)).toBe(false)
    expect(matchesSharingAge(unbounded, null)).toBe(false)
  })
  it('agrees with an independent frozen-schema oracle at 500/501/1000/1001 emoji and mixed boundaries', () => {
    const texts = [500, 501, 1000, 1001].map(n => '😀'.repeat(n)).concat(['e\u0301'.repeat(500), 'e\u0301'.repeat(500)+'Ａ', '😀'.repeat(499)+'e\u0301', ''])
    const rows = texts.map(text => ({ definition: 'SetIntro', value: { expected_revision: 0, text, age_bounds: unbounded, expires_at: null } }))
    const expected = JSON.parse(execFileSync('python3', [fileURLToPath(new URL('./contract-oracle.py', import.meta.url))], { input: JSON.stringify(rows), encoding: 'utf8' }))
    expect(rows.map(row => setIntroSchema.safeParse(row.value).success)).toEqual(expected)
    expect(expected).toEqual([true, true, true, false, true, false, true, true])
  })
  it('matches frozen public/private projection constraints independently', () => {
    const cases = [
      { definition: 'IntroPublic', value: source, schema: publicIntroSchema },
      { definition: 'IntroPublic', value: { ...source, text: '😀'.repeat(1000) }, schema: publicIntroSchema },
      { definition: 'IntroPublic', value: { ...source, text: '😀'.repeat(1001) }, schema: publicIntroSchema },
      { definition: 'IntroPublic', value: { ...source, age_bounds: unbounded }, schema: publicIntroSchema },
      { definition: 'IntroPrivate', value: own(), schema: privateIntroSchema },
      { definition: 'IntroPrivate', value: { ...own(), handle: 'private' }, schema: privateIntroSchema },
    ]
    const expected = JSON.parse(execFileSync('python3', [fileURLToPath(new URL('./contract-oracle.py', import.meta.url))], { input: JSON.stringify(cases.map(({ definition, value }) => ({ definition, value }))), encoding: 'utf8' }))
    expect(cases.map(({ schema, value }) => schema.safeParse(value).success)).toEqual(expected)
    expect(expected).toEqual([true, true, false, false, true, false])
  })
  it('strictly separates owner, public, age status and current error enums', () => {
    expect(publicIntroSchema.safeParse(source).success).toBe(true)
    expect(privateIntroSchema.safeParse(own()).success).toBe(true)
    for (const key of ['age', 'age_bounds', 'handle', 'claim', 'expires_at', 'owner_id']) expect(publicIntroSchema.safeParse({ ...source, [key]: null }).success).toBe(false)
    for (const key of ['age', 'handle', 'claim', 'tags']) expect(privateIntroSchema.safeParse({ ...own(), [key]: null }).success).toBe(false)
    expect(publicIntroSchema.safeParse({ ...source, author: { ...source.author, age: 20 } }).success).toBe(false)
    expect(publicIntroSchema.safeParse({ ...source, author: { ...source.author, display_name: '😀'.repeat(40), bio: '😀'.repeat(160) } }).success).toBe(true)
    expect(publicIntroSchema.safeParse({ ...source, author: { ...source.author, avatar: { ...source.author.avatar, claim: {} } } }).success).toBe(false)
    expect(publicIntroSchema.safeParse({ ...source, author: { ...source.author, avatar: { ...source.author.avatar, config: { ...source.author.avatar.config, handle: 'private' } } } }).success).toBe(false)
    for (const id of ['intro_123456789012', source.intro_id.toUpperCase().replace('0', 'A'), 'bad']) expect(privateIntroSchema.safeParse({ ...own(), intro_id: id }).success).toBe(false)
    const status = { user_id: source.author.user_id, event_id: source.event_id, status: 'UNKNOWN', verified_at: null, expires_at: null, verification_available: false }
    expect(ageStatusSchema.safeParse(status).success).toBe(true)
    expect(ageStatusSchema.safeParse({ ...status, age: 20 }).success).toBe(false)
    expect(ageStatusSchema.safeParse({ ...status, status: 'SELF_VERIFIED' }).success).toBe(false)
    for (const reason of ['INVALID_INPUT', 'EVENT_READ_ONLY', 'VERSION_CONFLICT']) expect(introWriteReasonSchema.safeParse(reason).success).toBe(true)
    for (const reason of ['AGE_NOT_ALLOWED', 'CONTACT_BLOCKED', 'SERVICE_UNAVAILABLE']) expect(introWriteReasonSchema.safeParse(reason).success).toBe(false)
  })
})
describe('publication-only contact policy', () => {
  it.each(['＠friend', 'ｈｔｔｐｓ：／／example.org', 'h t t p s : ∕ ∕ example.org', 'instagram。com/user', 'discord[.]gg/friend', 't\u200b.me/name', 'instagram dot com', 'line:id friend', '카카오톡', '０１０１２３４５６７８', '@name', '💡x.com', 'h\u0085ttps://example.org'])('detects %s', text => expect(introContactBypass(text)).toBe(true))
  it.each(['Enjoy the music', 'There are 12345678 seats', 'annex.com', 'context.me', 'Use the next line', '日本語と음악', 'Jazz & blues', '𝒜x.com', 'x.com𝒜', 'h\ufeffttps://example.org', 'x.com\u0345', '\u0345x.com', 'instagramd\u0345otcom'])('allows prose %s', text => expect(introContactBypass(text)).toBe(false))
  it('distinguishes empty drafts, publication/closed text, contact policy and expiry bounds', () => {
    expect(introPolicyValid(own(), context)).toBe(true)
    expect(introPolicyValid(own(), { ...context, publishing: true })).toBe(false)
    expect(introPolicyValid({ ...own(), state: 'CLOSED' }, context)).toBe(false)
    expect(introPolicyValid({ ...own(), text: '@friend' }, context)).toBe(true)
    expect(introPolicyValid({ ...own(), text: '@friend', state: 'PUBLISHED' }, context)).toBe(false)
    expect(introPolicyValid({ ...own(), text: '@friend', state: 'CLOSED' }, context)).toBe(true)
    expect(introPolicyValid({ ...own(), text: '\0' }, context)).toBe(false)
    expect(introPolicyValid({ ...own(), text: '\u0085', state: 'PUBLISHED' }, context)).toBe(false)
    expect(introPolicyValid({ ...own(), text: '\ufeff', state: 'PUBLISHED' }, context)).toBe(true)
    for (const expires of [context.now, context.eventCutoff + 1]) expect(introPolicyValid({ ...own(), expires_at: new Date(expires).toISOString() }, context)).toBe(false)
    expect(introPolicyValid({ ...own(), expires_at: new Date(context.eventCutoff).toISOString() }, context)).toBe(true)
    expect(introPolicyValid({ ...own(), created_at: new Date(context.now + 1).toISOString() }, context)).toBe(false)
    expect(introPolicyValid(own(), { ...context, eventCutoff: context.now })).toBe(false)
    expect(introPolicyValid({ ...own(), intro_id: '00000000-0000-0000-0000-000000000000' }, context)).toBe(false)
  })
})
