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
const styleBytes = await readFile(resolve(model, 'style-source.json'))
const style = JSON.parse(styleBytes)
const sha = bytes => 'sha256:' + createHash('sha256').update(bytes).digest('hex')
const artSource = 'af298e3246096c4b7a3ae5c7e439f5f33988a6be'
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
if (provenance.length !== 69597 || sha(provenance) !== 'sha256:0b9c5663e1fa182aae4012be465c448832ff798f926e930ee33c08cd456c084e') throw Error('Corrected artist provenance pin')
// Static advances from the exact original font, not a raster export or browser stand-in.
const retainedStyleBytes = await readFile('src/world/reference-scene/map4-service-v1/style-source.json')
const retainedStyle = JSON.parse(retainedStyleBytes)
if (sha(retainedStyleBytes) !== 'sha256:246562200c1379f901e08183bde53e57e03a5152eb0a03c62c4268319e890e62'
  || canonical(style.font) !== canonical(retainedStyle.font) || canonical(style.icons) !== canonical(retainedStyle.icons)
  || canonical(style.icon_cutouts) !== canonical(retainedStyle.icon_cutouts)) throw Error('Original v1/font/icons must remain exact')
const font = await readFile(style.font.path.replace(/^frontend\//, ''))
if (font.length !== style.font.bytes || sha(font) !== style.font.sha256) throw Error('Original font bytes')
function table(tag) {
  for (let i = 0; i < font.readUInt16BE(4); i++) { const p = 12 + i * 16
    if (font.toString('ascii', p, p + 4) === tag) return font.readUInt32BE(p + 8)
  }
  throw Error('Font table missing: ' + tag)
}
const units = font.readUInt16BE(table('head') + 18), hmtx = table('hmtx')
const metricCount = font.readUInt16BE(table('hhea') + 34), cmap = table('cmap')
let cmap4
for (let i = 0; i < font.readUInt16BE(cmap + 2); i++) {
  const row = cmap + 4 + i * 8, offset = cmap + font.readUInt32BE(row + 4)
  if (font.readUInt16BE(offset) === 4) { cmap4 = offset; break }
}
if (cmap4 === undefined) throw Error('Original uppercase font needs a format4 cmap')
function glyph(code) {
  const count = font.readUInt16BE(cmap4 + 6) / 2, ends = cmap4 + 14, starts = ends + count * 2 + 2
  const deltas = starts + count * 2, ranges = deltas + count * 2
  for (let i = 0; i < count; i++) if (code <= font.readUInt16BE(ends + i * 2)) {
    const start = font.readUInt16BE(starts + i * 2); if (code < start) break
    const delta = font.readInt16BE(deltas + i * 2), offset = font.readUInt16BE(ranges + i * 2)
    if (offset === 0) return (code + delta) & 65535
    const raw = font.readUInt16BE(ranges + i * 2 + offset + 2 * (code - start)); return raw ? (raw + delta) & 65535 : 0
  }
  return 0
}
const fontAdvanceMeasurements = serviceSemantics.map(label => {
  let advance = 0
  for (const character of label.label) {
    const id = glyph(character.charCodeAt(0)); if (!id) throw Error('Missing original label glyph')
    advance += font.readUInt16BE(hmtx + Math.min(id, metricCount - 1) * 4)
  }
  const textWidth = advance / units * style.style.font_size
  const paintWidth = Math.ceil(textWidth + style.style.padding * 2 + (label.icon ? style.style.icon_cell : 0) + style.style.text_metric_allowance)
  const source = JSON.parse(layout).serviceLabelAnchors.find(row => row.key === label.key)
  if (paintWidth > source.width || style.style.single_height > 78 || style.style.font_size * 265.5 / 768 < 13.8) throw Error('Compact paint would crop text or fall below planned short-view type size')
  return { key: label.key, fontSize: style.style.font_size, textAdvance: textWidth, paintWidth, paintHeight: style.style.single_height,
    originalHitWidth: source.width, originalHitHeight: 78, minimumShortTypePixels: style.style.font_size * 265.5 / 768 }
})
const presentationHash = sha(canonical({ version: compositionVersion, layout: resources.layout, assets: resources.assets, semantics: serviceSemantics, style }))
const registration = `// Deterministic registration of corrected ART af298e3246096c4b7a3ae5c7e439f5f33988a6be.\nimport resources from './resources.json'\nimport { compositionVersion } from './semantics'\nimport type { CompositionRegistration } from './contract'\n\nexport const compositionRegistration = { ...resources, version: compositionVersion,\n  presentationHash: '${presentationHash}' } as CompositionRegistration\n`
const current = `import layout from './layout-source.json'\nimport { createCompositionModel } from './model'\nimport { compositionRegistration } from './registration'\n\n// Immutable Map-only candidate; original release/World authority remains unchanged.\nconst model = createCompositionModel(layout, compositionRegistration)\nexport function currentComposition() { return model }\n`
for (const [name, content] of [['registration.ts', registration], ['current.ts', current]]) {
  const path = resolve(model, name)
  if (mode[0] === '--write' && name === 'registration.ts') await writeFile(path, content)
  else if (await readFile(path, 'utf8') !== content) throw Error('Registration postimage mismatch: ' + name)
}
process.stdout.write(JSON.stringify({ mode: mode[0], source: 'af298e3246096c4b7a3ae5c7e439f5f33988a6be', presentationHash,
  resources: rows, styleSha256: sha(styleBytes), fontAdvanceMeasurements, sharedInputPins, originalInputPins: 9, provenanceSha256: sha(provenance), publicArtifactsModified: false }) + '\n')
