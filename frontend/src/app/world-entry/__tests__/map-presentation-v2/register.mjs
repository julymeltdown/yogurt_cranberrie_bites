// Allocated text-only registration. Never exports, copies or modifies public artwork.
import { readFile, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { compositionVersion, serviceSemantics } from '../../../../world/reference-scene/map4-service-v2/semantics.ts'

const mode = process.argv.slice(2)
if (mode.length !== 1 || !['--write', '--check'].includes(mode[0])) throw Error('Use exactly --write or --check inside an allocated source round')
const deadline = Date.parse(process.env.MCR_MAP_SOURCE_DEADLINE ?? '')
if (!process.env.MCR_MAP_SOURCE_LEASE || !Number.isFinite(deadline)
  || Date.now() >= deadline - 60_000) throw Error('Fresh accepted source lease and cleanup reserve required')
const model = resolve('src/world/reference-scene/map4-service-v2')
const resources = JSON.parse(await readFile(resolve(model, 'resources.json'), 'utf8'))
const style = JSON.parse(await readFile('src/world/reference-scene/map4-service-v1/style-source.json', 'utf8'))
const sha = bytes => 'sha256:' + createHash('sha256').update(bytes).digest('hex')
const artSource = '8fcd1287496aedbde12ef0d2e426855f8f8ec4d6'
const gitBytes = path => execFileSync('git', ['show', artSource + ':' + path], { maxBuffer: 8 * 1024 * 1024 })
const sharedInputPins = []
for (const name of ['build.py', 'layout.json', 'source-provenance.json', 'README.md',
  'revenge-scene.svg', 'revenge-scene.png', 'parade-scene.svg', 'parade-scene.png', 'danger-scene.svg', 'danger-scene.png']) {
  const path = 'public/world/map-reference-composition-v1/' + name, bytes = await readFile(path)
  if (!bytes.equals(gitBytes('frontend/' + path))) throw Error('Read-only ART carry differs from committed source: ' + name)
  sharedInputPins.push({ path, bytes: bytes.length, sha256: sha(bytes) })
}
function canonical(value) {
  if (Array.isArray(value)) return '[' + value.map(canonical).join(',') + ']'
  if (value !== null && typeof value === 'object') return '{' + Object.keys(value).sort().map(k => JSON.stringify(k) + ':' + canonical(value[k])).join(',') + '}'
  return JSON.stringify(value)
}
if (resources.version !== compositionVersion || resources.assets.length !== 3) throw Error('Closed composition version/era count')
const rows = []
for (const pin of [resources.layout, ...resources.assets]) {
  if (!/^world\/map-reference-composition-v1\/(layout\.json|(revenge|parade|danger)-scene\.png)$/.test(pin.path)) throw Error('Closed resource path')
  const bytes = await readFile(resolve('public', pin.path))
  if (bytes.length !== pin.bytes || sha(bytes) !== pin.sha256) throw Error('Resource pin mismatch: ' + pin.path)
  if (pin.mime === 'image/png' && (bytes.readUInt32BE(16) !== 768 || bytes.readUInt32BE(20) !== 1296)) throw Error('Scene dimensions')
  rows.push({ path: pin.path, bytes: bytes.length, sha256: sha(bytes) })
}
const layout = await readFile(resolve('public', resources.layout.path))
if (!(await readFile(resolve(model, 'layout-source.json'))).equals(layout)) throw Error('Compiled layout snapshot must be byte-exact')
for (const pin of JSON.parse(layout).sourceInputs) {
  const bytes = gitBytes(pin.path)
  if (bytes.length !== pin.bytes || sha(bytes) !== pin.sha256) throw Error('Original input provenance mismatch: ' + pin.path)
}
const provenance = await readFile('public/world/map-reference-composition-v1/source-provenance.json')
if (provenance.length !== 26589 || sha(provenance) !== 'sha256:75a6391415528f2dd9a583aec51e74e6a35c9817c1c5985cb9d03477c080f338') throw Error('Corrected artist provenance pin')
const presentationHash = sha(canonical({ version: compositionVersion, layout: resources.layout, assets: resources.assets, semantics: serviceSemantics, style }))
const registration = `// Deterministic registration of corrected ART 8fcd1287496aedbde12ef0d2e426855f8f8ec4d6.\nimport resources from './resources.json'\nimport { compositionVersion } from './semantics'\nimport type { CompositionRegistration } from './contract'\n\nexport const compositionRegistration = { ...resources, version: compositionVersion,\n  presentationHash: '${presentationHash}' } as CompositionRegistration\n`
const current = `import layout from './layout-source.json'\nimport { createCompositionModel } from './model'\nimport { compositionRegistration } from './registration'\n\n// Immutable Map-only candidate; original release/World authority remains unchanged.\nconst model = createCompositionModel(layout, compositionRegistration)\nexport function currentComposition() { return model }\n`
for (const [name, content] of [['registration.ts', registration], ['current.ts', current]]) {
  const path = resolve(model, name)
  if (mode[0] === '--write') await writeFile(path, content)
  else if (await readFile(path, 'utf8') !== content) throw Error('Registration postimage mismatch: ' + name)
}
process.stdout.write(JSON.stringify({ mode: mode[0], source: '8fcd1287496aedbde12ef0d2e426855f8f8ec4d6', presentationHash,
  resources: rows, sharedInputPins, originalInputPins: 9, provenanceSha256: sha(provenance), publicArtifactsModified: false }) + '\n')
