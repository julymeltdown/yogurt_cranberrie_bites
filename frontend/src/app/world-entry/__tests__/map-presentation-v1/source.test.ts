import { execFileSync } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { expect, it } from 'vitest'
import bootstrap from './source-inputs/operator-bootstrap.json'
import style from '../../../../world/reference-scene/map4-service-v1/style-source.json'
import { map4ThemeHashes, map4Parade } from '../../map-presentation-v1/identity'

const base = '9185c99223bbf7437fff4346a7683dbb4187dfb0'
const root = new URL('../../../../../../', import.meta.url)
const git = (path: string) => execFileSync('git', ['show', base + ':' + path], { cwd: root })
const hash = (b: Uint8Array) => 'sha256:' + createHash('sha256').update(b).digest('hex')
it('preserves both copied public constructor inputs byte-for-byte and pins their actual all-era tuple', async () => {
  expect(await readFile(new URL('./source-inputs/operator-bootstrap.json', import.meta.url))).toEqual(git('backend/releases/operator-bundle/quality-v6/bootstrap.json'))
  expect(await readFile(new URL('./source-inputs/local-input.json', import.meta.url))).toEqual(git('backend/releases/operator-bundle/quality-v6/local-input.json'))
  expect(bootstrap.map_manifest.content_hash).toBe(map4Parade.mapHash)
  expect(bootstrap.map_manifest.map_revision).toBe(4)
  for (const theme of bootstrap.theme_manifests) {
    expect(theme.content_hash).toBe(map4ThemeHashes[theme.theme_id as keyof typeof map4ThemeHashes])
    expect(theme.theme_revision).toBe(1)
    expect(theme.scenery.ground.sha256).toBe('sha256:e2d9f785d31ea9e9a1888dc8f73d182eca917b54e1924fbf5159bc91be96e795')
  }
})
it('reuses only immutable artist font/icon/style sources, independently of v5 coordinates or admission', async () => {
  const bytes = await readFile(new URL('../../../../../public/world/reference-scene-v2/map-service-labels-v1/label-presentation.json', import.meta.url))
  expect(bytes.byteLength).toBe(17716)
  expect(hash(bytes)).toBe('sha256:3869218ae5c1c54c001e533f9a405a7f9fbfe6d5078e765daf51132aeab0aa26')
  const p = JSON.parse(bytes.toString())
  expect(style).toEqual({ style: p.style, font: p.font, icons: p.icons, icon_cutouts: p.icon_cutouts })
})
it('keeps the World renderer/camera/actor/geometry/runtime implementation unchanged from the received base', async () => {
  for (const path of ['frontend/src/world/runtime.ts', 'frontend/src/world/scenes/MapScene.ts',
    'frontend/src/world/reference-scene/spatial-v3/source-inputs.json',
    'frontend/src/world/reference-scene/spatial-v3/model.ts',
    'frontend/src/world/avatar/whole-character.ts',
    'frontend/src/app/world-entry/resolver.ts', 'frontend/src/app/world-entry/production.ts']) {
    const current = await readFile(new URL(path, root))
    expect(hash(current), path).toBe(hash(git(path)))
  }
})
