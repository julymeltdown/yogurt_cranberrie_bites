// SYNTHETIC TEST ART ONLY. No selected source is cropped, relabeled or replaced.
import avatar from '../../../../../../contracts/fixtures/assets/avatar-manifest.json'
import { testData as terrainData } from '../../../composition/__tests__/fixtures/data'
import { contentHash, sha256 } from '../../../map/json'
export async function testData(png: Uint8Array<ArrayBuffer>) {
  const data = await terrainData(), manifest = structuredClone(avatar)
  Object.assign(manifest.atlas.texture, { image_state: 'PROVIDED', path: 'world/fe12-test/avatar.png', sha256: await sha256(png) })
  manifest.content_hash = await contentHash(manifest)
  data.bootstrap.avatar_manifest = manifest
  for (const theme of data.bootstrap.theme_manifests) { theme.avatar_content_hash = manifest.content_hash; theme.content_hash = await contentHash(theme) }
  data.pin.avatar.content_hash = manifest.content_hash
  return { ...data, avatarPng: png, allow: [...data.allow, { path: 'world/fe12-test/avatar.png', sha256: await sha256(png), kind: 'png' as const }] }
}
