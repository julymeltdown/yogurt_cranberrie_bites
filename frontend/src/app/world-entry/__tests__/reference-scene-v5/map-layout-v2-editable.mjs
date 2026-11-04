import { readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
const escape = value => String(value).replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;')
export async function writeEditable(frontend, output, era) {
  const read = async p => JSON.parse(await readFile(resolve(frontend, p), 'utf8'))
  const base = 'src/world/reference-scene/spatial-v5/'
  const source = await read(base + 'source-inputs.json'), labels = await read(base + 'map-labels-source.json'), plants = await read(base + 'map-landscape-source.json')
  const labelPin = await read(base + 'map-labels-pins.json'), plantPin = await read(base + 'map-landscape-pin.json')
  const data = source.scenes[era], scenery = data.scenery, atlas = scenery.atlases[0]
  const png = async descriptor => 'data:image/png;base64,' + (await readFile(resolve(frontend, 'public', descriptor.path))).toString('base64')
  const font = (await readFile(resolve(frontend, labels.font.path.replace('frontend/', '')))).toString('base64')
  const replacements = new Set(source.overview_representations.flatMap(o => o.replaces_prop_id ? [o.replaces_prop_id] : []))
  const rows = [...scenery.objects.filter(o => !replacements.has(o.id)), ...[...source.overview_representations, ...plants.decorations].map(o => ({ ...o, frame: data.frames[o.frame] }))].sort((a, b) => a.depth_y - b.depth_y)
  const elements = [`<image width="768" height="1296" href="${await png(scenery.ground)}"/>`, `<polygon points="${source.plaza_parcel.map(p => p.join(',')).join(' ')}" fill="#7a8742" stroke="#b4a27d" stroke-width="6"/>`]
  for (const o of rows) {
    const f = o.frame
    elements.push(`<svg data-object="${escape(o.id)}" data-depth="${o.depth_y}" x="${o.x}" y="${o.y}" width="${o.width}" height="${o.height}" viewBox="${f.x} ${f.y} ${f.width} ${f.height}" preserveAspectRatio="none" overflow="hidden"><use href="#atlas"/></svg>`)
  }
  const s = labels.style
  for (const l of labels.labels) {
    const r = l.rect, b = s.border_width, x = r.x + s.padding, y = r.y + (r.height - s.icon_size) / 2, k = s.icon_size / 24
    elements.push(`<g data-label="${escape(l.label)}" data-anchor="${l.x},${l.y}"><rect data-paper="" x="${r.x}" y="${r.y}" width="${r.width}" height="${r.height}" fill="${s.border_fill}"/><rect x="${r.x + b}" y="${r.y + b}" width="${r.width - 2*b}" height="${r.height - 2*b}" fill="${s.paper_fill}"/>`)
    if (l.icon) {
      elements.push(`<rect x="${x - 3}" y="${y - 3}" width="${s.icon_size + 6}" height="${s.icon_size + 6}" fill="${s.icon_paper}"/>`)
      for (const [shapes, color] of [[labels.icons[l.icon], s.icon_fill], [labels.icon_cutouts[l.icon] ?? [], s.icon_paper]]) for (const shape of shapes) elements.push(`<polygon points="${shape.map(([xx, yy]) => [x + xx*k, y + yy*k].join(',')).join(' ')}" fill="${color}"/>`)
    }
    l.lines.forEach((line, i) => elements.push(`<text x="${r.x+s.padding+(l.icon ? s.icon_cell : 0)}" y="${r.y+(r.height-l.lines.length*s.line_height)/2+32+i*s.line_height}" font-family="MCR Map Labels" font-size="${s.font_size}" fill="${s.text_fill}">${escape(line)}</text>`))
    elements.push('</g>')
  }
  const metadata = { format: 'MCR_MAP_LAYOUT_V2_EDITABLE_SVG', era, geometryHash: source.geometry_hash, layoutPair: labelPin.layout_pair, documentHashes: [labelPin.document_sha256, plantPin.document_sha256], authority: 'Editable MAP derivative only; no World/admission/target-raster pixels', objects: rows.length, labels: 6 }
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="768" height="1296" viewBox="0 0 768 1296" style="image-rendering:pixelated"><metadata>${escape(JSON.stringify(metadata))}</metadata><defs><style>@font-face{font-family:'MCR Map Labels';src:url(data:font/ttf;base64,${font})}</style><image id="atlas" width="${atlas.width_px}" height="${atlas.height_px}" href="${await png(atlas)}"/></defs>${elements.join('')}</svg>`
  await writeFile(resolve(output, `editable-${era.toLowerCase()}.svg`), svg + '\n')
  return { svg, metadata }
}
