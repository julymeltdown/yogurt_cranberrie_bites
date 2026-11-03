// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest'
import { createWorldEntryResolver } from '../../world-entry/resolver'
import { assetBytes, authHarness, bundle, inputConfiguration, requestFor, seal } from '../../world-entry/__tests__/data'
const owners: ReturnType<typeof authHarness>[] = []
afterEach(() => owners.splice(0).forEach(owner => owner.auth.dispose()))
it.each(['unreviewed', 'missing', 'foreign-hash'] as const)('does not admit optional %s theme bytes or authorize selection', async mode => {
 const h = authHarness(); owners.push(h)
 const b = await bundle(), theme = b.theme_manifests.find(t => t.theme_id === 'REVENGE')!
 if (mode === 'unreviewed') Object.assign(theme.provenance, {review_state:'UNREVIEWED', reviewed_by:null, reviewed_at:null})
 if (mode === 'missing') Object.assign(theme.texture, {image_state:'NOT_PROVIDED',path:null,sha256:null})
 await seal(b)
 const request = await requestFor(h.auth, Promise.resolve(b)); let calls = 0
 const result = await createWorldEntryResolver({auth:h.auth,origin:'https://mcr.test',localInput:inputConfiguration,assetFetch:async input => {
  calls++; const response = new Response(assetBytes.png,{headers:{'Content-Type':'image/png'}});Object.defineProperty(response,'url',{value:String(input)});return response
 }})(request)
 if (mode !== 'foreign-hash') expect(result.availableThemeIds).not.toContain('REVENGE')
 const resource = {path:theme.texture.path ?? 'world/test-only/revenge.png',sha256:mode === 'foreign-hash' ? 'sha256:'+'0'.repeat(64) : theme.texture.sha256 ?? 'sha256:'+'0'.repeat(64),kind:'png' as const}
 await expect(result.resources.read(resource,request.signal)).rejects.toThrow();expect(calls).toBe(0)
})
