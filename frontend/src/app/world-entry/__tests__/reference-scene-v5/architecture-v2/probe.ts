import revenge from '../../../../../../public/world/reference-scene-v2/architecture-v2/revenge/scenery.json'
import parade from '../../../../../../public/world/reference-scene-v2/architecture-v2/parade/scenery.json'
import danger from '../../../../../../public/world/reference-scene-v2/architecture-v2/danger/scenery.json'
import source from '../../../../../world/reference-scene/spatial-v5/source-inputs.json'
import labels from '../../../../../world/reference-scene/spatial-v5/map-labels-source.json'
import plants from '../../../../../world/reference-scene/spatial-v5/map-landscape-source.json'
import { verifySpatialV5MapLayoutPair } from '../../../../../world/reference-scene/spatial-v5/model'
// @ts-expect-error Independent pinned artist label oracle is JavaScript.
import { drawLabels } from '../../../../../world/reference-scene/__tests__/spatial-v5/label-reference.js'

/** Independent source composition; does not read production binding objects or expected pixels. */
export async function probeArchitectureMap(imageUrl: string, era: keyof typeof source.scenes) {
  await verifySpatialV5MapLayoutPair()
  const invalid = []
  for (const fault of ['mixed-pair', 'anchor']) {
    const l = structuredClone(labels), p = structuredClone(plants)
    if (fault === 'mixed-pair') p.required_label_presentation.sha256 = 'sha256:' + '0'.repeat(64)
    else { l.labels[0]!.x++; l.labels[0]!.rect.x++ }
    try { await verifySpatialV5MapLayoutPair(l, p); invalid.push(false) } catch { invalid.push(true) }
  }
  const data = { ...source.scenes[era], scenery: { REVENGE: revenge, PARADE: parade, DANGER: danger }[era] }, textures = new Map<string, HTMLImageElement>(), decoded: HTMLImageElement[] = []
  const load = async (url: string) => { const image = new Image(); image.src = url; await image.decode(); decoded.push(image); return image }
  for (const item of [data.scenery.ground, ...data.scenery.atlases]) textures.set(item.id, await load('/' + item.path))
  const actual = await load(imageUrl), a = document.createElement('canvas'), b = document.createElement('canvas')
  a.width = b.width = 768; a.height = b.height = 1296
  const x = a.getContext('2d')!, y = b.getContext('2d')!; x.drawImage(actual, 0, 0); y.imageSmoothingEnabled = false
  y.drawImage(textures.get(data.scenery.ground.id)!, 0, 0)
  y.beginPath(); source.plaza_parcel.forEach(([x, z], i) => i ? y.lineTo(x!, z!) : y.moveTo(x!, z!)); y.closePath()
  y.fillStyle = '#7a8742'; y.fill(); y.strokeStyle = '#b4a27d'; y.lineWidth = 6; y.stroke()
  const replaced = new Set(source.overview_representations.flatMap(p => 'replaces_prop_id' in p ? [p.replaces_prop_id] : []))
  const frames = data.frames as Record<string, { x: number; y: number; width: number; height: number }>
  const rows = [...data.scenery.objects.filter(o => !replaced.has(o.id)), ...[...source.overview_representations, ...plants.decorations].map(o => ({ ...o, frame: frames[o.frame]!, atlas_id: data.scenery.atlases[0]!.id }))]
  for (const o of rows.sort((a, b) => a.depth_y - b.depth_y)) {
    const f = o.frame; y.drawImage(textures.get(o.atlas_id)!, f.x, f.y, f.width, f.height, o.x, o.y, o.width, o.height)
  }
  drawLabels(y, 1)
  const aa = x.getImageData(0, 0, 768, 1296).data, bb = y.getImageData(0, 0, 768, 1296).data
  let different = 0
  for (let i = 0; i < aa.length; i++) if (aa[i] !== bb[i]) different++
  a.width = a.height = b.width = b.height = 0; decoded.forEach(image => { image.src = '' })
  return { differentChannels: different, channels: aa.length, objects: rows.length, invalidPairsRejected: invalid, labels: labels.labels.length, decorations: plants.decorations.length }
}
