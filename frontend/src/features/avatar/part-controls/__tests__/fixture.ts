import { readFile } from 'node:fs/promises'
import { URL as FileURL } from 'node:url'
import { vi } from 'vitest'
import { setup } from '../../__tests__/reviewed-production/fixtures'
import { object } from '../../../auth/model/contract'
import { createMotionRuntime } from '../../../../entities/avatar/runtime/motion-adoption/runtime'
import { MOTION_ART } from '../../../../entities/avatar/runtime/motion-adoption/art'
import { createReviewedMotionEditorOwner } from '../../motion-adoption/production-owner'

/** Deterministic canonical C03 transport + real admitted runtime and frozen bytes; native image decode is mocked. */
export async function paletteFixture(canSave = true) {
  const f = await setup()
  if (!canSave) object(f.bundle().capabilities).mutations = { available: false, reason: 'FEATURE_DISABLED' }
  const authority = await f.create()
  const runtime = createMotionRuntime({ authority, scope: f.scope, privacy: f.auth.privacy, origin: location.origin,
    fetch: async input => {
      const url = String(input), path = new URL(url).pathname.slice(1)
      if (path !== MOTION_ART.descriptor.path && path !== MOTION_ART.atlas.path) throw Error('Unexpected resource')
      const bytes = new Uint8Array(await readFile(new FileURL(`../../../../../public/${path}`, import.meta.url)))
      const response = new Response(bytes, { headers: { 'Content-Type': path.endsWith('.png') ? 'image/png' : 'application/json', 'Content-Length': String(bytes.length) } })
      Object.defineProperty(response, 'url', { value: url }); return response
    },
  })
  const owner = createReviewedMotionEditorOwner({ auth: f.auth, scope: f.scope, authority, runtime })
  owner.start(); await vi.waitFor(() => {
    const state = owner.controller.read()
    if (state.phase !== 'ready' || !state.persisted || !state.catalog) throw Error(`Editor ${state.phase}/${state.notice}`)
  })
  return { ...f, authority, runtime, owner, async wardrobe() {
    return owner.controller.choose({ mode: 'wardrobe', body: 'body-0', hair: 'hair-5', eyes: 'eyes-3', outfit: 'outfit-4', accessory: null })
  }, dispose() { owner.dispose(); runtime.dispose(); f.dispose() } }
}
