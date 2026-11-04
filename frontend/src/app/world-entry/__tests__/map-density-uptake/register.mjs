// Read-only registration qualification; never rebuilds or rewrites artist output.
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { compositionVersion, serviceSemantics } from '../../../../world/reference-scene/map4-service-v2/semantics.ts'
const sha = bytes => 'sha256:' + createHash('sha256').update(bytes).digest('hex')
const canonical = value => Array.isArray(value) ? '[' + value.map(canonical).join(',') + ']'
  : value !== null && typeof value === 'object' ? '{' + Object.keys(value).sort().map(k => JSON.stringify(k) + ':' + canonical(value[k])).join(',') + '}' : JSON.stringify(value)
const prefix = 'public/world/map-density-derived-v1/', model = 'src/world/reference-scene/map4-service-v2/'
const resources = JSON.parse(await readFile(model + 'resources.json'))
const style = JSON.parse(await readFile(model + 'style-source.json'))
assert.equal(resources.version, 'map-density-derived-v1'); assert.equal(resources.version, compositionVersion)
const readset = JSON.parse(await readFile('../docs/mcr/evidence/map-density-uptake/readset.json'))
for (const pin of readset.pins) assert.equal(sha(await readFile('../' + pin.path)), 'sha256:' + pin.sha256, pin.path)
const frozen = []
for (const name of ['README.md', 'build.py', 'layout.json', 'source-provenance.json', ...['revenge','parade','danger'].flatMap(e => [e+'-scene.png',e+'-scene.svg'])]) {
  const path = prefix + name, raw = await readFile(path)
  assert(raw.equals(execFileSync('git', ['show', '7acfac3aed91f1eb06fa48eee63ce3da1431fadc:frontend/' + path], { maxBuffer: 8 * 1024**2 })), path)
  frozen.push({ path, bytes: raw.length, sha256: sha(raw) })
}
for (const pin of [resources.layout, ...resources.assets]) {
  const raw = await readFile('public/' + pin.path)
  assert.equal(raw.length, pin.bytes); assert.equal(sha(raw), pin.sha256)
}
assert((await readFile(model + 'layout-source.json')).equals(await readFile(prefix + 'layout.json')))
const expected = sha(canonical({ version: compositionVersion, layout: resources.layout, assets: resources.assets, semantics: serviceSemantics, style }))
const registration = await readFile(model + 'registration.ts', 'utf8')
assert.equal(registration.match(/presentationHash:\s*'(sha256:[a-f0-9]{64})'/)?.[1], expected)
console.log(JSON.stringify({ status: 'PASS', presentationHash: expected, frozenArtistSource: '5d2d0d34536fa8bf9314c67e5a3b8390e9baf553', canonicalPack: '7acfac3aed91f1eb06fa48eee63ce3da1431fadc', readsetPins: readset.pins.length, frozen, artworkModified: false }))
