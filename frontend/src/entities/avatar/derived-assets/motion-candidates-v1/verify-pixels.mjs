import { decode } from './pixels.mjs'
/** Independent readback checks. Does not call the crop/register implementation. */
export async function verifyPixels(manifest) {
  const sources = new Map(), base = '/world/avatars/derived-v1/motion-candidates-v1/', reports = []
  const atlas = await decode(base + manifest.atlas.file)
  for (const [index, item] of manifest.items.entries()) {
    if (!sources.has(item.sourceId)) sources.set(item.sourceId, await decode('/world/avatars/supplied-v1/photo-' + item.sourceId.slice(-2) + '.jpg'))
    const original = sources.get(item.sourceId), native = await decode(base + item.native.file), low = await decode(base + item.registered.file)
    let opaque = 0, white = 0, dark = 0, rgbErrors = 0, sampleErrors = 0, atlasErrors = 0, borderPixels = 0, nonBinaryAlpha = 0
    const assert = (condition, message) => { if (!condition) throw Error(item.sourceId + '/' + item.location + ': ' + message) }
    assert(native.width === item.rect.width && native.height === item.rect.height, 'native dimensions')
    for (let y = 0; y < native.height; y++) for (let x = 0; x < native.width; x++) {
      const p = (y * native.width + x) * 4, a = native.rgba[p + 3]
      if (a !== 0 && a !== 255) nonBinaryAlpha++
      if (!a) continue
      opaque++; if (!x || !y || x === native.width - 1 || y === native.height - 1) borderPixels++
      const q = ((item.rect.y + y) * original.width + item.rect.x + x) * 4
      for (let c = 0; c < 3; c++) if (native.rgba[p + c] !== original.rgba[q + c]) rgbErrors++
      if (Math.min(...native.rgba.subarray(p, p + 3)) >= 230) white++
      if (Math.max(...native.rgba.subarray(p, p + 3)) <= 112) dark++
    }
    const t = item.registered
    for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) {
      const p = (y * 64 + x) * 4, a = low.rgba[p + 3]
      if (a !== 0 && a !== 255) nonBinaryAlpha++
      if (a && (!x || !y || x === 63 || y === 63)) borderPixels++
      const sx = Math.floor((x + .5 - t.targetAnchor.x) / t.scale + t.sourceAnchor.x), sy = Math.floor((y + .5 - t.targetAnchor.y) / t.scale + t.sourceAnchor.y)
      const inside = sx >= 0 && sy >= 0 && sx < native.width && sy < native.height
      const expected = inside ? native.rgba.subarray((sy * native.width + sx) * 4, (sy * native.width + sx) * 4 + 4) : [0,0,0,0]
      if (a !== expected[3] || (a && [0,1,2].some(c => low.rgba[p + c] !== expected[c]))) sampleErrors++
      const q = ((Math.floor(index / 8) * 64 + y) * atlas.width + index % 8 * 64 + x) * 4
      for (let c = 0; c < 4; c++) if (atlas.rgba[q + c] !== low.rgba[p + c]) atlasErrors++
    }
    assert(opaque === item.native.stats.opaque && white === item.native.stats.white && dark === item.native.stats.dark, 'retained content counts')
    assert(item.native.stats.discardedDark === 0, 'source dark outline was discarded')
    assert(!rgbErrors && !sampleErrors && !atlasErrors && !borderPixels && !nonBinaryAlpha, 'source/registration/atlas/alpha/border mismatch')
    reports.push({ sourceId: item.sourceId, location: item.location, opaque, white, dark, rgbErrors, sampleErrors, atlasErrors, borderPixels, nonBinaryAlpha })
  }
  const protectedDetails = []
  for (const [id, x, y, label, minChannel] of [['photo_02',150,236,'eye highlight',230],['photo_04',841,80,'pale split hair',180],['photo_04',1124,310,'pale hair shadow',110],['photo_19',1041,68,'white headwear',180]]) {
    const item = manifest.items.find(p => p.sourceId === id && x >= p.rect.x && x < p.rect.x + p.rect.width && y >= p.rect.y && y < p.rect.y + p.rect.height)
    const native = await decode(base + item.native.file), p = ((y - item.rect.y) * native.width + x - item.rect.x) * 4
    if (native.rgba[p + 3] !== 255 || Math.min(...native.rgba.subarray(p,p+3)) < minChannel) throw Error('Protected light detail missing: ' + label + JSON.stringify({ x,y,rgba:Array.from(native.rgba.subarray(p,p+4)) }))
    protectedDetails.push({ sourceId:id,x,y,label,rgba:Array.from(native.rgba.subarray(p,p+4)) })
  }
  return { reports, protectedDetails, atlas: { width:atlas.width,height:atlas.height }, fullAvatarDirections:manifest.fullAvatarDirections, walkFrames:manifest.walkFrames }
}
