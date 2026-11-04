import assert from 'node:assert/strict'
import { readFile, writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { pathToFileURL } from 'node:url'
import { candidateServer,root,publicPath as output,evidencePath as evidence } from './server.mjs'
const catalog = JSON.parse(await readFile(root + 'frontend/src/entities/avatar/character-source-pack.json', 'utf8'))
const hair = JSON.parse(await readFile(root + 'frontend/src/entities/avatar/derived-assets/hair-layout.json', 'utf8'))
const hash = b => createHash('sha256').update(b).digest('hex')
assert(process.env.MCR_ART_PLAYWRIGHT, 'Set MCR_ART_PLAYWRIGHT to retained installed @playwright/test/index.mjs')
const { chromium } = await import(pathToFileURL(process.env.MCR_ART_PLAYWRIGHT).href)
const originals = []
for (const f of [...catalog.files, ...catalog.archived_files]) { const b = await readFile(root + 'frontend/public/world/avatars/supplied-v1/' + f.file); assert.equal(hash(b), f.sha256); assert.equal(b.length, f.byte_length); originals.push({ sourceId: f.source_id, sha256: hash(b), bytes: b.length }) }
const server = candidateServer()
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
let browser
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const page = await browser.newPage(); await page.goto(`http://127.0.0.1:${server.address().port}/tools/pixels.mjs`)
  const result = await page.evaluate(async ({ hair, catalog }) => {
    const { decode, cut, register, pack, png, sheetGrid } = await import('/tools/pixels.mjs')
    const items = [], registered = [], sources = new Map(), failures = []
    for (const entry of catalog.files) sources.set(entry.source_id, await decode('/world/avatars/supplied-v1/' + entry.file))
    const regions = []
    for (let row = 0; row < 2; row++) for (let col = 0; col < 4; col++) regions.push({ sourceId: 'photo_02', location: `row${row + 1}-col${col + 1}`, role: 'front-body-baked-eyes-and-underclothes', rect: { x: 35 + col * 301, y: 95 + row * 395, width: 295, height: 370 } })
    for (const s of hair.sources) for (const shape of s.shapes) regions.push({ sourceId: s.source_id, location: shape.source_location.replaceAll('_', '-'), role: s.source_id === 'photo_04' ? 'front-hair-layer' : 'rear-hair-reference-unpaired', rect: shape.rect })
    for (const [sourceId, role, columns, rows] of [
      ['photo_06', 'outfit-reference-baked-hands-not-skin-independent', 4, 4],
      ['photo_07', 'bottoms-reference-baked-legs-not-skin-independent', 4, 4],
      ['photo_08', 'eyewear-layer-unfitted', 6, 3],
      ['photo_09', 'eye-pair-layer-unfitted', 3, 6],
      ['photo_17', 'eyes-brows-expression-layer-unfitted', 4, 4],
      ['photo_19', 'headwear-layer-unfitted', 4, 4],
      ['photo_20', 'wearable-accessory-layer-unfitted', 4, 4],
    ]) {
      let grid
      try { grid = sheetGrid(sources.get(sourceId), columns, rows) } catch (error) { throw Error(sourceId + ': ' + error.message) }
      for (let row = 0; row < rows; row++) for (let col = 0; col < columns; col++) {
        const x = grid.xRows[row][col], y = grid.y[row]
        regions.push({ sourceId, location: `row${row + 1}-col${col + 1}`, role, rect: { x, y, width: grid.xRows[row][col + 1] - x + 1, height: grid.y[row + 1] - y + 1 }, grid })
      }
    }
    // The prop rows overlap vertically across columns. Explicit reviewed windows
    // replace the contact-sheet grid; every crop still fails on clipped art.
    const props = [[70,5,240,265],[328,20,300,240],[658,15,270,254],[935,0,345,295],
      [30,266,290,245],[330,280,300,230],[650,269,270,243],[930,300,340,210],
      [40,509,280,253],[340,507,290,248],[631,510,304,250],[933,510,332,230],
      [40,767,290,190],[330,755,310,204],[651,759,291,200],[933,740,335,219]]
    props.forEach(([x,y,width,height], i) => regions.push({ sourceId: 'photo_21', location: `row${Math.floor(i / 4) + 1}-col${i % 4 + 1}`, role: 'handheld-prop-layer-unfitted-no-entitlement', rect: { x,y,width,height }, reviewedWindow: true }))
    for (const region of regions) {
      const source = sources.get(region.sourceId)
      let part
      try { part = cut(source, region.rect) } catch (error) { failures.push(region.sourceId + '/' + region.location + ': ' + error.message); continue }
      const b = part.bounds
      const body = region.sourceId === 'photo_02', rear = region.sourceId === 'photo_05'
      const hairPart = ['photo_04', 'photo_05'].includes(region.sourceId)
      const scale = body ? 50 / b.height : hairPart ? .145 : Math.min(56 / b.width, 56 / b.height)
      const sourceAnchor = body ? { x: b.x + b.width / 2, y: b.y + b.height } : hairPart ? { x: b.x + b.width / 2, y: b.y + b.height * .45 } : { x: b.x + b.width / 2, y: b.y + b.height / 2 }
      const targetAnchor = body ? { x: 32, y: 60 } : hairPart ? { x: 32, y: 21 } : { x: 32, y: 32 }
      const low = register(part, sourceAnchor, targetAnchor, scale)
      items.push({ ...region, native: { png: png(part.rgba, part.width, part.height), width: part.width, height: part.height, bounds: b, stats: part.stats }, registered: { png: png(low.rgba, 64, 64), kind: body ? 'BODY_FOOT' : hairPart && !rear ? 'FRONT_CRANIUM_CANDIDATE' : 'PART_REFERENCE_NOT_AVATAR_ATTACHMENT', ...low.transform, stats: low.stats }, suppliedView: rear ? 'REAR_HAIR_ONLY_NOT_FULL_AVATAR_DIRECTION' : 'FRONT_ONLY', productItemId: null, pairedFrontBack: null })
      registered.push(low)
    }
    if (failures.length) throw Error(failures.join('\n'))
    const contact = document.createElement('canvas'); contact.width = 1024; contact.height = 20 * 12 + 5 * 192
    const ctx = contact.getContext('2d'); ctx.fillStyle = '#ded9cd'; ctx.fillRect(0, 0, contact.width, contact.height); ctx.imageSmoothingEnabled = false
    for (const [i, entry] of catalog.files.entries()) {
      const source = sources.get(entry.source_id), c = document.createElement('canvas'); c.width = source.width; c.height = source.height
      c.getContext('2d').putImageData(new ImageData(source.rgba, source.width, source.height), 0, 0)
      const x = i % 4 * 256, y = Math.floor(i / 4) * 240
      ctx.drawImage(c, x, y + 20, 256, source.height / source.width * 256); ctx.fillStyle = '#171717'; ctx.font = '14px sans-serif'; ctx.fillText(entry.source_id + ' ' + entry.category, x + 4, y + 16)
    }
    return { items, atlas: pack(registered), contact: contact.toDataURL('image/png').split(',')[1] }
  }, { hair, catalog })
  const files = []
  const savePng = async (name, base64) => { const bytes = Buffer.from(base64, 'base64'), sha256 = hash(bytes), file = `${name}-${sha256.slice(0, 16)}.png`; await writeFile(root + output + file, bytes); files.push({ file, sha256, bytes: bytes.length }); return file }
  for (const item of result.items) { item.native.file = await savePng(item.sourceId.replace('_', '-') + '-' + item.location + '-native', item.native.png); delete item.native.png; item.registered.file = await savePng(item.sourceId.replace('_', '-') + '-' + item.location + '-64', item.registered.png); delete item.registered.png }
  result.atlas.file = await savePng('source-parts-64', result.atlas.png); delete result.atlas.png
  await writeFile(root + evidence + 'selected-source-contact.png', Buffer.from(result.contact, 'base64'))
  const manifest = { kind: 'SELECTED_SOURCE_MOTION_LAYER_CANDIDATES', revision: 1, status: 'IN_PROGRESS', productionEligible: false, sourcePack: { id: catalog.source_pack_id, revision: catalog.revision }, sourceInputs: originals, baseCommit: 'a3526254a716e14e3134de551c3b277379a1096f', fullAvatarDirections: [], walkFrames: [], noPoseSynthesis: true, bodyFoot: { x: 32, y: 60 }, frameSize: { width: 64, height: 64 }, items: result.items, atlas: result.atlas, files }
  await writeFile(root + output + 'candidate-index.json', JSON.stringify(manifest, null, 2) + '\n')
  await writeFile(root + evidence + 'original-hashes.json', JSON.stringify({ originals, allMatch: true, sourceCatalogUnchanged: true }, null, 2) + '\n')
  console.log(JSON.stringify({ result: 'EXPORTED_CANDIDATES_NOT_ADMITTED', parts: result.items.length, pngs: files.length, originalHashes: originals.length }))
} finally { await browser?.close(); await new Promise(resolve => server.close(resolve)) }
