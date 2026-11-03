// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import selected from '../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { bundle, authHarness, requestFor, inputConfiguration } from '../../world-entry/__tests__/data'
import { contentHash } from '../../../world/map/json'
import { createWorldEntryResolver } from '../../world-entry/resolver'
import { decodeBootstrap } from '../../../features/auth/model/bootstrap'

const stops: (() => void)[] = []
afterEach(() => stops.splice(0).forEach(stop => stop()))
async function fixture() {
  const old = await bundle(), avatar = { revision: 7, config: structuredClone(selected.presets[4]!.config) }
  const themes = await Promise.all(old.theme_manifests.map(async source => {
    const theme = { ...source, avatar_manifest_id: selected.manifest_id, avatar_manifest_version: selected.manifest_version, avatar_content_hash: selected.content_hash }
    theme.content_hash = await contentHash(theme); return theme
  }))
  const data = { ...old, avatar, avatar_manifest: selected, theme_manifests: themes,
    participant: { ...old.participant, profile: { ...old.participant.profile, avatar } } }
  const h = authHarness(); stops.push(h.auth.dispose)
  const previous = await requestFor(h.auth)
  const request = { ...previous, bootstrap: await decodeBootstrap(data, previous.scope) }
  return { ...h, request, data }
}
it('admits the exact reviewed whole release through the default resolver without invented layered provenance', async () => {
  const f = await fixture(), resolve = createWorldEntryResolver({ auth: f.auth, origin: 'https://mcr.test', localInput: inputConfiguration })
  expect('provenance' in selected).toBe(false)
  const result = await resolve(f.request)
  expect(result.bootstrap).toBe(f.request.bootstrap.document)
  expect(result.privacyScope).toEqual(f.request.privacyScope)
  expect(result.bootstrap).toMatchObject({ avatar_manifest: selected })
  expect(f.requests).toHaveLength(1)
  expect(new URL(f.requests[0]!.url).pathname.endsWith('/rooms')).toBe(true)
})
it('rejects an altered whole release before acquiring any rooms authority', async () => {
  const f = await fixture(), resolve = createWorldEntryResolver({ auth: f.auth, origin: 'https://mcr.test', localInput: inputConfiguration })
  const mutated = { ...f.data, avatar_manifest: { ...selected, content_hash: `sha256:${'0'.repeat(64)}` } }
  await expect(decodeBootstrap(mutated, f.request.scope).then(bootstrap => resolve({ ...f.request, bootstrap }))).rejects.toThrow()
  expect(f.requests).toHaveLength(0)
})
