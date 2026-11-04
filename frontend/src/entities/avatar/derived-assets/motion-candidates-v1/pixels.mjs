/** Selected-source candidate preparation. No redraw, recoloring or inferred poses. */
export async function decode(path) {
  const image = new Image(); image.src = path; await image.decode()
  const canvas = document.createElement('canvas'); canvas.width = image.naturalWidth; canvas.height = image.naturalHeight
  const ctx = canvas.getContext('2d', { willReadFrequently: true }); ctx.drawImage(image, 0, 0)
  return { width: canvas.width, height: canvas.height, rgba: ctx.getImageData(0, 0, canvas.width, canvas.height).data }
}
export function png(rgba, width, height) {
  const c = document.createElement('canvas'); c.width = width; c.height = height
  c.getContext('2d').putImageData(new ImageData(rgba, width, height), 0, 0)
  return c.toDataURL('image/png').split(',')[1]
}
/** Flood only perimeter-connected neutral paper. Enclosed whites are retained.
 * Every opaque RGB is copied from the browser's original JPEG decode. */
export function cut(source, rect) {
  const { x: ox, y: oy, width: w, height: h } = rect, rgba = new Uint8ClampedArray(w * h * 4)
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) rgba.set(source.rgba.subarray(((oy + y) * source.width + ox + x) * 4, ((oy + y) * source.width + ox + x) * 4 + 4), (y * w + x) * 4)
  const background = new Uint8Array(w * h), queue = []
  const add = p => { const rgb = rgba.subarray(p * 4, p * 4 + 3), lo = Math.min(...rgb), hi = Math.max(...rgb)
    if (!background[p] && lo >= 118 && hi - lo <= 18) { background[p] = 1; queue.push(p) } }
  for (let x = 0; x < w; x++) { add(x); add((h - 1) * w + x) }
  for (let y = 0; y < h; y++) { add(y * w); add(y * w + w - 1) }
  for (let i = 0; i < queue.length; i++) { const p = queue[i], x = p % w, y = Math.floor(p / w)
    if (x) add(p - 1); if (x + 1 < w) add(p + 1); if (y) add(p - w); if (y + 1 < h) add(p + w) }
  // JPEG ringing can create isolated non-paper specks. Keep all components with
  // an actual dark outline, including disconnected paired accessories.
  const visited = new Uint8Array(w * h), keep = new Uint8Array(w * h), parts = []
  for (let p = 0; p < w * h; p++) {
    if (background[p] || visited[p]) continue
    const q = [p]; visited[p] = 1; let dark = 0
    for (let i = 0; i < q.length; i++) { const n = q[i], x = n % w, y = Math.floor(n / w)
      if (Math.max(...rgba.subarray(n * 4, n * 4 + 3)) <= 112) dark++
      for (const next of [x ? n - 1 : -1, x + 1 < w ? n + 1 : -1, y ? n - w : -1, y + 1 < h ? n + w : -1]) if (next >= 0 && !background[next] && !visited[next]) { visited[next] = 1; q.push(next) } }
    if (dark > 0) { q.forEach(n => { keep[n] = 1 }); parts.push({ pixels: q.length, dark }) }
  }
  let minX = w, minY = h, maxX = -1, maxY = -1, opaque = 0, white = 0, dark = 0, discardedDark = 0
  for (let p = 0; p < w * h; p++) { rgba[p * 4 + 3] = keep[p] ? 255 : 0
    const isDark = Math.max(...rgba.subarray(p * 4, p * 4 + 3)) <= 112
    if (!keep[p]) { if (isDark) discardedDark++; continue }
    minX = Math.min(minX, p % w); maxX = Math.max(maxX, p % w); minY = Math.min(minY, Math.floor(p / w)); maxY = Math.max(maxY, Math.floor(p / w)); opaque++
    if (Math.min(...rgba.subarray(p * 4, p * 4 + 3)) >= 230) white++
    if (isDark) dark++
  }
  if (!opaque || minX === 0 || minY === 0 || maxX === w - 1 || maxY === h - 1) throw Error('Missing or clipped foreground ' + JSON.stringify({ rect, minX, minY, maxX, maxY }))
  return { rgba, width: w, height: h, bounds: { x: minX, y: minY, width: maxX - minX + 1, height: maxY - minY + 1 }, stats: { opaque, white, dark, discardedDark, removedPaper: queue.length, parts } }
}
export function register(part, sourceAnchor, targetAnchor, scale) {
  const rgba = new Uint8ClampedArray(64 * 64 * 4); let opaque = 0, white = 0
  for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) {
    const sx = Math.floor((x + .5 - targetAnchor.x) / scale + sourceAnchor.x), sy = Math.floor((y + .5 - targetAnchor.y) / scale + sourceAnchor.y)
    if (sx < 0 || sy < 0 || sx >= part.width || sy >= part.height) continue
    const i = (sy * part.width + sx) * 4
    if (!part.rgba[i + 3]) continue
    rgba.set(part.rgba.subarray(i, i + 4), (y * 64 + x) * 4); opaque++
    if (Math.min(...part.rgba.subarray(i, i + 3)) >= 230) white++
  }
  return { rgba, width: 64, height: 64, stats: { opaque, white }, transform: { sourceAnchor, targetAnchor, scale, uniform: true, sampling: 'NEAREST_SOURCE_PIXEL' } }
}
export function pack(parts, columns = 8) {
  const width = columns * 64, height = Math.ceil(parts.length / columns) * 64, rgba = new Uint8ClampedArray(width * height * 4)
  for (const [i, part] of parts.entries()) for (let y = 0; y < 64; y++) rgba.set(part.rgba.subarray(y * 64 * 4, (y + 1) * 64 * 4), ((Math.floor(i / columns) * 64 + y) * width + i % columns * 64) * 4)
  return { png: png(rgba, width, height), width, height }
}
/** Contact-sheet guides are hints. Move partitions onto real paper gutters. */
export function sheetGrid(source, columns, rows) {
  const axis = (length, count, horizontal, start = 0, end = horizontal ? source.height : source.width) => {
    const cuts = [0]
    for (let i = 1; i < count; i++) {
      const center = Math.round(length * i / count), radius = 70, scores = []
      for (let n = center - radius; n <= center + radius; n++) {
        let dark = 0
        for (let q = start; q < end; q++) {
          const p = ((horizontal ? q : n) * source.width + (horizontal ? n : q)) * 4
          if (Math.max(...source.rgba.subarray(p, p + 3)) <= 112) dark++
        }
        scores.push({ n, dark })
      }
      const min = Math.min(...scores.map(s => s.dark)), runs = []
      for (const s of scores) if (s.dark === min) {
        const last = runs.at(-1)
        if (last && last.end === s.n - 1) last.end = s.n
        else runs.push({ start: s.n, end: s.n })
      }
      const chosen = runs.sort((a, b) => Math.abs((a.start + a.end) / 2 - center) - Math.abs((b.start + b.end) / 2 - center))[0]
      if (min !== 0) throw Error('No source-paper gutter at guide ' + center)
      cuts.push(Math.floor((chosen.start + chosen.end) / 2))
    }
    cuts.push(length - 1); return cuts
  }
  const y = axis(source.height, rows, false)
  return { y, xRows: y.slice(0, -1).map((start, row) => axis(source.width, columns, true, start, y[row + 1] + 1)) }
}
