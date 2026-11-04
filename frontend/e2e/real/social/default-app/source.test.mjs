import assert from 'node:assert/strict'
import test from 'node:test'
import { mkdtempSync, mkdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { bindServingTree, bridgePaths, frozenBridge, frozenSource, productionPath, releaseHelpers, servingTreeCurrent, sha256 } from './source.mjs'

test('frozen actual-main readset pins the requested bridge and excludes old harness/tests', () => {
  const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../../../../..'), proof = frozenSource(repository, frozenBridge)
  for (const path of bridgePaths) assert(proof.files.some(file => file.path === path))
  assert(proof.files.every(file => !file.path.includes('/__tests__/') && !file.path.includes('/fixtures/')
    && (!file.path.startsWith('frontend/e2e/') || releaseHelpers.includes(file.path))))
  assert.equal(proof.manifestSha256.length, 64)
  // A legacy Git candidate cannot stand in for the frozen app, independent of the physical checkout.
  assert.throws(() => frozenSource(repository, 'f00017b2d8386ce10a607de869fcecb18e3945ec'))
})
test('borrowed file guard rejects mutation/symlinks and does not rewrite a frozen source tree', () => {
  const root = mkdtempSync(resolve(tmpdir(), 'mcr-default-source-'))
  try {
    mkdirSync(resolve(root, 'frontend/src'), { recursive: true })
    const path = resolve(root, 'frontend/src/main.tsx'); writeFileSync(path, 'unit frozen bytes')
    const proof = { files: [{ path: 'frontend/src/main.tsx', sha256: sha256('unit frozen bytes') }] }
    const bound = bindServingTree(proof, root)
    assert.equal(servingTreeCurrent(bound), true)
    writeFileSync(path, 'changed'); assert.equal(servingTreeCurrent(bound), false); assert.throws(() => bindServingTree(proof, root))
    rmSync(path); writeFileSync(resolve(root, 'foreign'), 'unit frozen bytes'); symlinkSync(resolve(root, 'foreign'), path)
    assert.throws(() => bindServingTree(proof, root))
  } finally { rmSync(root, { recursive: true }) }
})
test('source path admission excludes traversal, fixture entrypoints and unrelated private files', () => {
  for (const path of ['frontend/e2e/real/social/browser-state.ts', 'frontend/src/../private', 'docs/private.json', 'frontend/src/app/__tests__/entry.tsx', 'contracts/http/v1/fixtures/synthetic.json']) assert.equal(productionPath(path), false)
  assert.equal(productionPath('frontend/src/main.tsx'), true)
})
