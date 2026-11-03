import type { ActiveScope, ShellBootstrap } from '../../../app/ports'
import { projectCapabilities } from '../../../app/capabilities'
import { canonicalJson, decodeContract, equal, list, number, object, requireValue, text, type ObjectValue } from './contract'

export { canonicalJson } from './contract'
function freezeDocument<T>(value: T): T {
  if (value !== null && typeof value === 'object') {
    for (const child of Object.values(value)) freezeDocument(child)
    Object.freeze(value)
  }
  return value
}
async function digest(value: unknown) {
  const result = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(canonicalJson(value)))
  return `sha256:${Array.from(new Uint8Array(result), b => b.toString(16).padStart(2, '0')).join('')}`
}
function unique(entries: unknown, key: string) {
  const rows = list(entries).map(object), index = new Map(rows.map(v => [text(v[key]), v]))
  requireValue(index.size === rows.length)
  return index
}
const sameSet = (a: unknown[], b: unknown[]) => a.length === b.length && a.every(v => b.includes(v))
async function validateBundle(bootstrap: ObjectValue) {
  const avatar = object(bootstrap.avatar_manifest), map = object(bootstrap.map_manifest)
  const themes = unique(bootstrap.theme_manifests, 'theme_id')
  requireValue(sameSet([...themes.keys()], ['REVENGE', 'PARADE', 'DANGER']))
  await Promise.all([avatar, map, ...themes.values()].map(async document => {
    const { content_hash, ...body } = document
    requireValue(content_hash === await digest(body))
  }))
  const atlas = object(avatar.atlas), texture = object(atlas.texture)
  const items = unique(atlas.items, 'item_id'), tones = unique(avatar.skin_tones, 'skin_tone_id'), presets = unique(avatar.presets, 'preset_id')
  const layers = ['body', 'eyes', 'hair', 'outfit', 'accessory']
  requireValue(presets.has(text(avatar.fallback_preset_id))
    && sameSet([...new Set([...items.values()].map(item => item.layer_id))], layers))
  requireValue(number(texture.width_px) % 64 === 0 && number(texture.height_px) % 64 === 0)
  const occupied = new Set<string>()
  for (const item of items.values()) {
    const r = object(item.region), x = number(r.x), y = number(r.y), w = number(r.width_px), h = number(r.height_px)
    requireValue(x % 64 === 0 && y % 64 === 0 && x + w <= number(texture.width_px) && y + h <= number(texture.height_px))
    for (let row = y / 64; row < (y + h) / 64; row++) for (let col = x / 64; col < (x + w) / 64; col++) {
      const cell = `${col},${row}`; requireValue(!occupied.has(cell)); occupied.add(cell)
    }
  }
  for (const config of [object(object(bootstrap.avatar).config), ...[...presets.values()].map(p => object(p.config))]) {
    requireValue(config.manifest_id === avatar.manifest_id && config.manifest_version === avatar.manifest_version
      && tones.has(text(config.skin_tone_id)))
    for (const [layer, id] of Object.entries(object(config.item_ids))) {
      if (layer === 'accessory' && id === null) continue
      requireValue(items.get(text(id))?.layer_id === layer)
    }
  }
  const geometry = object(map.geometry), width = number(geometry.width_cells), height = number(geometry.height_cells)
  requireValue(map.geometry_hash === await digest(geometry))
  for (const name of ['collision_grid', 'terrain_layer', 'overdraw_layer']) {
    const grid = list(geometry[name]); requireValue(grid.length === height && grid.every(row => list(row).length === width))
  }
  const places = unique(map.places, 'place_id'), spawns = unique(geometry.spawns, 'spawn_id'), interactions = unique(geometry.interactions, 'interaction_id')
  requireValue(spawns.has(text(geometry.default_spawn_id)))
  const collision = list(geometry.collision_grid).map(list)
  const walkable = (x: number, y: number) => x >= 0 && y >= 0 && x < width && y < height && collision[y]?.[x] === 0
  for (const place of places.values()) if (object(place.provenance).source_kind === 'ORIGINAL_FIXTURE') {
    requireValue(place.verification_state === 'CONCEPT_ONLY' && place.place_kind === 'VIRTUAL_CONCEPT')
  }
  for (const entry of [...spawns.values(), ...interactions.values()]) requireValue(walkable(number(entry.cell_x), number(entry.cell_y)))
  for (const interaction of interactions.values()) requireValue(places.has(text(interaction.place_id)))
  // One connected-component traversal suffices for the frozen undirected collision grid.
  const first = [...spawns.values()][0]!, queue = [[number(first.cell_x), number(first.cell_y)]], seen = new Set<string>()
  for (let i = 0; i < queue.length; i++) {
    const [x = 0, y = 0] = queue[i]!, key = `${x},${y}`
    if (seen.has(key)) continue
    seen.add(key)
    for (const [nx = 0, ny = 0] of [[x - 1, y], [x + 1, y], [x, y - 1], [x, y + 1]]) {
      if (walkable(nx, ny) && !seen.has(`${nx},${ny}`)) queue.push([nx, ny])
    }
  }
  for (const entry of [...spawns.values(), ...interactions.values()]) requireValue(seen.has(`${entry.cell_x},${entry.cell_y}`))
  for (const theme of themes.values()) {
    const bindings = { map_id: map.map_id, map_revision: map.map_revision, map_content_hash: map.content_hash,
      geometry_hash: map.geometry_hash, collision_id: geometry.collision_id, avatar_manifest_id: avatar.manifest_id,
      avatar_manifest_version: avatar.manifest_version, avatar_content_hash: avatar.content_hash }
    requireValue(Object.entries(bindings).every(([k, v]) => theme[k] === v))
    requireValue(sameSet(list(theme.place_ids), [...places.keys()]) && sameSet(list(theme.interaction_ids), [...interactions.keys()]))
    const t = object(theme.texture), tw = number(geometry.tile_width_px), th = number(geometry.tile_height_px)
    requireValue(number(t.width_px) % tw === 0 && number(t.height_px) % th === 0)
    const count = number(t.width_px) / tw * (number(t.height_px) / th)
    for (const name of ['terrain_layer', 'overdraw_layer']) requireValue(list(geometry[name]).every(row => list(row).every(gid => number(gid) <= count)))
  }
}
export async function decodeBootstrap(value: unknown, scope: ActiveScope): Promise<ShellBootstrap> {
  // Own the exact validated bytes before asynchronous hash checks. A caller
  // cannot mutate an asset/default after validation or during its awaits.
  const b = freezeDocument(decodeContract('Bootstrap', structuredClone(value))), event = object(b.event), participant = object(b.participant)
  const profile = object(participant.profile), capabilities = object(b.capabilities)
  requireValue(event.event_id === scope.eventId && participant.event_id === scope.eventId
    && participant.user_id === scope.userId && profile.user_id === scope.userId && equal(profile.avatar, b.avatar))
  requireValue(!['READ_ONLY', 'ARCHIVED'].includes(text(event.phase)) || object(capabilities.mutations).available === false)
  await validateBundle(b)
  return Object.freeze({ contractVersion: '0.1.0', event: Object.freeze({ eventId: scope.eventId,
    title: text(event.title), phase: text(event.phase) as ShellBootstrap['event']['phase'] }),
  participant: Object.freeze({ userId: scope.userId, eventId: scope.eventId }), worldAvailable: object(capabilities.world).available === true,
  capabilities: projectCapabilities(capabilities), document: b })
}
