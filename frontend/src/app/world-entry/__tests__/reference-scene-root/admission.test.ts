// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { sceneData, localInput, userId, eventId } from './data'
import { authHarness } from '../data'
import { decodeBootstrap } from '../../../../features/auth/model/bootstrap'
import { createWorldEntryResolver } from '../../resolver'
import { preflightReferenceScene } from '../../reference-scene-overview'
import { validateManifests } from '../../../../world/map/manifest'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).reverse().forEach(stop => stop()))
it.each([{ version: 'legacy', era: 'PARADE' }, { version: 'spatial', era: 'REVENGE' }, { version: 'spatial', era: 'PARADE' }, { version: 'spatial', era: 'DANGER' }] as const)('preserves $version/$era provenance and records actual default source-art denial before HTTP', async ({ version, era }) => {
  const data = await sceneData(version, era), h = authHarness(); cleanup.push(h.auth.dispose)
  const scope = { userId, eventId, generation: h.auth.session.read().generation }
  const request = { scope, privacyScope: h.auth.privacy.capture(), bootstrap: await decodeBootstrap(data.bootstrap, scope), signal: new AbortController().signal }
  const fetch = vi.fn<typeof globalThis.fetch>()
  const metadata = await validateManifests(JSON.stringify(data.map), JSON.stringify(data.theme), data.pin)
  expect(() => preflightReferenceScene(metadata)).not.toThrow()
  expect(data.map.places.every(p => p.place_kind === 'VIRTUAL_CONCEPT' && p.verification_state === 'CONCEPT_ONLY')).toBe(true)
  expect(data.map.provenance.review_state).toBe('UNREVIEWED'); expect(data.theme.provenance.review_state).toBe('UNREVIEWED')
  await expect(createWorldEntryResolver({ auth: h.auth, origin: 'https://mcr.test', localInput, assetFetch: fetch })(request)).rejects.toThrow()
  expect(h.requests).toHaveLength(0); expect(fetch).not.toHaveBeenCalled()
  // Current art-source guard behavior, not a rule requiring physical verification for virtual places.
})
