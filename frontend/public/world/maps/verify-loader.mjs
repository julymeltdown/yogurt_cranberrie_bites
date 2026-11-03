// Execute the actual immutable FE14 Tiled parser without modifying application files.
import { readFile, writeFile, mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve, join, dirname } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { createRequire } from 'node:module'
import { createHash } from 'node:crypto'
import assert from 'node:assert/strict'
const here = dirname(fileURLToPath(import.meta.url)), root = resolve(here, '../../../..')
const require = createRequire(resolve(process.env.MCR_NODE_MODULE_ROOT ?? '/Users/lhs/Desktop/mcr/frontend/node_modules', '../package.json'))
const ts = require('typescript')
const temporary = await mkdtemp(join(tmpdir(), 'mcr-ar02-loader-'))
const sourceHashes = {}
try {
  for (const name of ['json', 'tiled']) {
    const path = `frontend/src/world/map/${name}.ts`, source = await readFile(join(root, path), 'utf8')
    sourceHashes[path] = createHash('sha256').update(source).digest('hex')
    const js = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } }).outputText.replaceAll("from './json'", "from './json.mjs'")
    await writeFile(join(temporary, `${name}.mjs`), js)
  }
  const { validateTiled } = await import(pathToFileURL(join(temporary, 'tiled.mjs')))
  const geometry = JSON.parse(await readFile(join(here, 'geometry.json')))
  const tiled = JSON.parse(await readFile(join(here, 'courtyard-tiled.json')))
  const texture = { path: 'world/themes/revenge.png', width_px: 256, height_px: 128 }
  validateTiled(JSON.stringify(tiled), geometry, texture)
  const rejected = []
  for (const [name, mutate] of [
    ['collision-drift', t => { t.layers[1].data[0] = 0 }],
    ['extra-object-layer', t => { t.layers.push({ type: 'objectgroup', name: 'spawns' }) }],
    ['external-tileset', t => { t.tilesets[0].source = 'https://example.invalid/tiles.tsx' }],
    ['wrong-atlas-dimensions', t => { t.tilesets[0].imagewidth = 128 }],
    ['flipped-gid', t => { t.layers[0].data[0] = 2147483649 }],
  ]) {
    const bad = structuredClone(tiled); mutate(bad)
    assert.throws(() => validateTiled(JSON.stringify(bad), geometry, texture), name)
    rejected.push(name)
  }
  const report = { status: 'PASS', scope: 'Actual FE14 validateTiled plus strict JSON parser on original AR02 exported resources', node: process.version, typescript: ts.version,
    sourceHashes, accepted: 'courtyard-tiled.json', rejected, limits: ['No application composition or final manifest pin', 'No actual server runtime or physical devices'] }
  await writeFile(join(root, 'docs/mcr/art/evidence/maps/loader-validation.json'), JSON.stringify(report, null, 2) + '\n')
  console.log(JSON.stringify(report, null, 2))
} finally { await rm(temporary, { recursive: true, force: true }) }
