import { isMap4Presentation, map4Labels, type Map4PresentationIdentity } from './annotations'

/** Readable local MAP scale on short screens; scroll instead of shrinking 44px actions. */
export function map4LocalZoom(slot: { width: number; height: number }, zoom: number) {
  const baseHeight = Math.max(slot.width * 1296 / 768, slot.height)
  return Math.max(448 / baseHeight, zoom)
}
export function map4ServiceTargets(identity: Map4PresentationIdentity, width: number, height: number) {
  if (!isMap4Presentation(identity)) return []
  return map4Labels.filter(l => l.intent !== null).map(label => {
    const w = Math.max(44, label.rect.width / 768 * width), h = Math.max(44, label.rect.height / 1296 * height)
    return { intent: label.intent!, label: label.label,
      left: Math.max(0, Math.min(width - w, (label.rect.x + label.rect.width / 2) / 768 * width - w / 2)),
      top: Math.max(0, Math.min(height - h, (label.rect.y + label.rect.height / 2) / 1296 * height - h / 2)), width: w, height: h }
  })
}

type Rect = { left: number; top: number; width: number; height: number }
/** Compact canonical markers stay separate from virtual feature hit regions. */
export function packMap4Markers(anchors: readonly { id: string; x: number; y: number }[], width: number, height: number, excluded: readonly Rect[], options: { preferAnchorProximity?: boolean } = {}) {
  const placed: Rect[] = [], result = new Map<string, { x: number; y: number }>()
  const overlaps = (a: Rect, b: Rect, gap = 4) => a.left < b.left + b.width + gap && a.left + a.width + gap > b.left
    && a.top < b.top + b.height + gap && a.top + a.height + gap > b.top
  const markerRect = (p: { x: number; y: number }): Rect => ({ left: p.x - 22, top: p.y - 22, width: 44, height: 44 })
  function edgeCandidates(anchor: typeof anchors[number], obstacles: readonly Rect[], gap = 4) {
    const clearance = 22 + gap + .01
    const xs = [...new Set([22, width - 22, anchor.x,
      ...obstacles.flatMap(r => [r.left - clearance, r.left + r.width + clearance])])].filter(x => x >= 22 && x <= width - 22)
    const ys = [...new Set([22, height - 22, anchor.y,
      ...obstacles.flatMap(r => [r.top - clearance, r.top + r.height + clearance])])].filter(y => y >= 22 && y <= height - 22)
    return xs.flatMap(x => ys.map(y => ({ x, y }))).filter(p => obstacles.every(e => !overlaps(markerRect(p), e, gap)))
      .sort((a, b) => (a.x - anchor.x) ** 2 + (a.y - anchor.y) ** 2 - (b.x - anchor.x) ** 2 - (b.y - anchor.y) ** 2)
  }
  function solve(remaining: typeof anchors, occupied: readonly Rect[], gap: number): Map<string, { x: number; y: number }> | null {
    if (!remaining.length) return new Map()
    const choices = remaining.map(anchor => ({ anchor, points: edgeCandidates(anchor, [...excluded, ...occupied], gap) }))
      .sort((a, b) => a.points.length - b.points.length)
    const next = choices[0]!
    for (const point of next.points) {
      const tail = solve(remaining.filter(a => a !== next.anchor), [...occupied, markerRect(point)], gap)
      if (tail) { tail.set(next.anchor.id, point); return tail }
    }
    return null
  }
  // Public catalog order need not follow visual order. Reserve upper landmarks
  // first so a lower neighbor cannot take their only nearby free corridor.
  const packingOrder = options.preferAnchorProximity ? [...anchors].sort((a, b) => a.y - b.y || a.x - b.x) : anchors
  for (const anchor of packingOrder) {
    const candidates: { x: number; y: number }[] = []
    for (let y = 24; y <= height - 24; y += 48) for (let x = 24; x <= width - 24; x += 48) candidates.push({ x, y })
    candidates.sort((a, b) => (a.x - anchor.x) ** 2 + (a.y - anchor.y) ** 2 - (b.x - anchor.x) ** 2 - (b.y - anchor.y) ** 2)
    const available = (p: { x: number; y: number }) => {
      const r = { left: p.x - 22, top: p.y - 22, width: 44, height: 44 }
      return [...excluded, ...placed].every(e => !overlaps(r, e))
    }
    let point = candidates.find(available)
    if (options.preferAnchorProximity) {
      const edge = edgeCandidates(anchor, [...excluded, ...placed])[0]
      const distance = (p: { x: number; y: number }) => (p.x - anchor.x) ** 2 + (p.y - anchor.y) ** 2
      if (edge && (!point || distance(edge) < distance(point))) point = edge
    }
    if (!point) {
      // Retain incumbent grid positions whenever possible. Narrow free corridors
      // need candidate centers at actual obstacle edges rather than a finer grid.
      point = edgeCandidates(anchor, [...excluded, ...placed])[0]
    }
    if (!point) {
      // Greedy incumbents may occupy the only corridor available to a later
      // marker. Repack the small canonical set, most constrained marker first.
      // At the minimum scrollable scale, optional cosmetic clearance may not
      // fit. Keep full 44px, non-overlapping hit regions even in that case.
      const solved = solve(anchors, [], 4) ?? solve(anchors, [], 0)
      if (!solved) throw Error('Map4 marker space unavailable')
      return new Map(anchors.map(a => [a.id, solved.get(a.id)!]))
    }
    result.set(anchor.id, point); placed.push({ left: point.x - 22, top: point.y - 22, width: 44, height: 44 })
  }
  return new Map(anchors.map(a => [a.id, result.get(a.id)!]))
}
