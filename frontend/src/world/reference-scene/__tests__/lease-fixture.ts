// Metadata/ownership-only injected decoded lease. Actual PNG pixels are qualified by native.mjs.
import { vi } from 'vitest'
import { referenceFixture } from './fixture'
import { validateManifests } from '../../map/manifest'
import type { LoadedMap } from '../../map/resources'
import { referenceScenery } from '../model'
export async function decodedFixture(): Promise<LoadedMap> {
  const f = await referenceFixture(), metadata = await validateManifests(JSON.stringify(f.map), JSON.stringify(f.theme), f.pin)
  const descriptors = [referenceScenery.ground, ...referenceScenery.atlases]
  return { ...metadata, image: { naturalWidth: metadata.theme.texture.width_px, naturalHeight: metadata.theme.texture.height_px } as HTMLImageElement,
    scenery: { manifest: metadata.theme.scenery!, assets: descriptors.map(descriptor => ({ descriptor,
      image: { naturalWidth: descriptor.width_px, naturalHeight: descriptor.height_px } as HTMLImageElement })),
      transferBytes: 1947817, decodedBytes: 8175616 }, dispose: vi.fn() }
}
