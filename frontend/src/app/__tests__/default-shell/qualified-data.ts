// TEST ONLY: simulated original-review metadata around geometric fixtures. Never product art.
import original from '../../../../../contracts/fixtures/core/bootstrap-open.json'
import { syntheticRelease } from '../../../world/theme/__tests__/fixtures/releases'
import { seal } from '../../world-entry/__tests__/data'
import { characterSourcePack } from '../../../entities/avatar/source-catalog'
export async function qualifiedData() {
 const releases = await Promise.all((['REVENGE','PARADE','DANGER'] as const).map(syntheticRelease))
 const b = structuredClone(original)
 b.map_manifest = structuredClone(releases[0]!.map) as typeof b.map_manifest
 b.theme_manifests = releases.map(f=>structuredClone(f.theme)) as typeof b.theme_manifests
 b.participant.preferences.theme='REVENGE'
 for(const capability of Object.values(b.capabilities)) Object.assign(capability,{available:true,reason:null})
 for(const manifest of [b.map_manifest,b.avatar_manifest,...b.theme_manifests]) Object.assign(manifest.provenance,{source_kind:'ORIGINAL_ART',source_id:manifest===b.avatar_manifest?characterSourcePack.source_pack_id:'test_only',source_revision:'test-only',review_state:'REVIEWED',reviewed_by:'Test only',reviewed_at:'2026-11-03T00:00:00Z'})
 for(const place of b.map_manifest.places) Object.assign(place.provenance,{source_kind:'ORIGINAL_ART',review_state:'REVIEWED',reviewed_by:'Test only',reviewed_at:'2026-11-03T00:00:00Z'})
 Object.assign(b.avatar_manifest.atlas.texture,{image_state:'PROVIDED',path:'world/default-test/avatar.png',sha256:releases[0]!.theme.texture.sha256})
 await seal(b)
 return {bootstrap:b,releases}
}
