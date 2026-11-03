import type { WorldRendererPort, WorldRendererLease, WorldRendererStatus } from '../composition'
import type { ReplicationView } from '../network/replication'
import { policy } from '../network/transport/contract'
import { frameIndex, type AvatarManifest } from '../../entities/avatar/runtime/contract'
import type { AvatarCompositor, ComposedAvatar } from '../../entities/avatar/runtime/compositor'
import type { AvatarSceneSprite, AvatarSceneTexture } from './scene'
import { canonicalJson } from '../map/json'

/** Generic engine adapter. Production entry supplies the reviewed loader in index.ts. */
export function createAvatarRenderer(options: { load(manifest: Readonly<Record<string, unknown>>, signal: AbortSignal): Promise<AvatarCompositor>; walkFrameMs?: number }) {
  const walkFrameMs = options.walkFrameMs ?? 120
  if (!Number.isFinite(walkFrameMs) || walkFrameMs < 50 || walkFrameMs > 1000) throw new Error('Invalid avatar cadence')
  let active: { read(): Readonly<{ status: WorldRendererStatus; sprites: number; textures: number; frames: number; fallback: number }> } | null = null
  let mounts = 0, disposals = 0
  const port: WorldRendererPort = { mount(context) {
    mounts++
    let disposed = false, status: WorldRendererStatus = 'loading', compositor: AvatarCompositor | null = null
    let latest: ReplicationView | null = null, scopeKey: string | null = null, sampling = false, frames = 0, fallback = 0
    const controller = new AbortController(), listeners = new Set<() => void>(), scene = context.avatarScene
    type Texture = { composed: ComposedAvatar; texture: AvatarSceneTexture; refs: number }
    type Row = { sprite: AvatarSceneSprite; texture: Texture; configKey: string; fallback: boolean }
    const textures = new Map<string, Texture>(), rows = new Map<string, Row>()
    let removeFrame = () => {}
    const owner = { read: () => ({ status, sprites: rows.size, textures: textures.size, frames, fallback }) }
    active = owner
    const current = () => !disposed && !context.signal.aborted && !controller.signal.aborted
    function setStatus(next: WorldRendererStatus) {
      if (!current() || status === next) return
      status = next; for (const listener of Array.from(listeners)) { if (!current()) break; try { listener() } catch { /* Independent status observers. */ } }
    }
    function remove(id: string, row: Row) {
      row.sprite.dispose(); rows.delete(id)
      if (--row.texture.refs === 0) {
        row.texture.texture.dispose(); row.texture.composed.release(); textures.delete(row.texture.composed.key)
      }
    }
    function clear() { for (const [id, row] of Array.from(rows)) remove(id, row) }
    function eligible(view: ReplicationView | null) {
      return view && view.visible && ['live', 'switching'].includes(view.phase) && view.baseline && view.entities.length <= policy.room_capacity_max
    }
    function render(time: number) {
      if (!current() || !scene || !compositor || !eligible(latest)) { clear(); return }
      const view = latest!, ids = new Set(view.entities.map(e => e.player.userId))
      if (ids.size !== view.entities.length) { clear(); setStatus('unavailable'); return }
      const keys = new Map(view.entities.map(e => [e.player.userId, canonicalJson(e.player.avatar.config)]))
      for (const [id, row] of Array.from(rows)) if (!ids.has(id) || row.configKey !== keys.get(id)) remove(id, row)
      let unavailable = false; fallback = 0
      for (const entity of view.entities) {
        if (!current()) return
        const { player, position } = entity
        let acquired: ComposedAvatar | null = null, created: AvatarSceneTexture | null = null
        try {
          if (!Number.isFinite(position.x) || !Number.isFinite(position.y) || position.x < 0 || position.y < 0
            || position.x > context.map.widthPx || position.y > context.map.heightPx) throw new Error('Invalid avatar position')
          let row = rows.get(player.userId)
          if (!row) {
            acquired = compositor.acquire(player.avatar.config)
            const usedFallback = acquired.fallback
            let texture = textures.get(acquired.key)
            if (texture) { acquired.release(); acquired = null }
            else { created = scene.texture(acquired.canvas); texture = { composed: acquired, texture: created, refs: 0 }; textures.set(acquired.key, texture); acquired = null; created = null }
            const sprite = scene.sprite(texture.texture)
            row = { sprite, texture, configKey: keys.get(player.userId)!, fallback: usedFallback }; texture.refs++; rows.set(player.userId, row)
          }
          if (row.fallback) fallback++
          row.sprite.update(position.x, position.y, frameIndex({ direction: player.facing,
            animation: player.moving ? 'walk' : 'idle', frame: player.moving ? Math.floor(time / walkFrameMs) % 4 : 0 }),
            1 + position.y / (context.map.heightPx + 1) * .9)
        } catch {
          created?.dispose(); acquired?.release()
          const row = rows.get(player.userId); if (row) remove(player.userId, row)
          // A sprite allocation failure may leave an unreferenced newly added texture.
          for (const [key, texture] of Array.from(textures)) if (texture.refs === 0) { texture.texture.dispose(); texture.composed.release(); textures.delete(key) }
          unavailable = true
        }
      }
      setStatus(unavailable ? 'unavailable' : 'ready')
    }
    function replace(view: ReplicationView) {
      if (!current()) return
      const key = JSON.stringify([view.scope.userId, view.scope.eventId, view.scope.generation])
      if (view.scope.eventId !== context.release.resources.eventId || scopeKey !== null && key !== scopeKey) { dispose(); return }
      scopeKey ??= key; latest = view
      if (!eligible(view)) { clear(); return }
      // Removals are synchronous even when called during sample() from the scene frame.
      const ids = new Set(view.entities.map(e => e.player.userId))
      for (const [id, row] of Array.from(rows)) if (!ids.has(id)) remove(id, row)
      if (!sampling) render(performance.now())
    }
    function dispose() {
      if (disposed) return
      disposed = true; disposals++; controller.abort(); context.signal.removeEventListener('abort', dispose)
      removeFrame(); clear(); compositor?.dispose(); compositor = null; latest = null; listeners.clear()
      if (active === owner) active = null
    }
    context.signal.addEventListener('abort', dispose, { once: true })
    if (context.signal.aborted) dispose()
    if (current() && scene) {
      removeFrame = scene.onFrame(time => {
        if (!current()) return
        frames++; sampling = true
        try { const view = context.sample(); if (!current()) return; if (view) replace(view); else { latest = null; clear() } }
        catch { latest = null; clear(); setStatus('unavailable') }
        finally { sampling = false }
        render(time)
      })
      void Promise.resolve().then(() => {
        if (!current()) return null
        return options.load(context.release.avatarManifest, controller.signal)
      }).then(loaded => {
        if (!loaded) return
        if (!current()) { loaded.dispose(); return }
        compositor = loaded
        if ((loaded.manifest as AvatarManifest).content_hash !== context.release.avatarManifest.content_hash) { loaded.dispose(); compositor = null; setStatus('unavailable'); return }
        setStatus('ready'); if (current()) render(performance.now())
      }, () => { if (current()) setStatus('unavailable') })
    } else if (current()) setStatus('unavailable')
    const lease: WorldRendererLease = { replace, dispose, readStatus: () => disposed ? 'unavailable' : status,
      subscribeStatus(listener) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } } }
    return lease
  } }
  return { ...port, read: () => ({ ...(active?.read() ?? { status: 'unavailable' as const, sprites: 0, textures: 0, frames: 0, fallback: 0 }), mounts, disposals }) }
}
