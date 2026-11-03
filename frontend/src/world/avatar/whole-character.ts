import type { WholeCharacterLease, WholeCharacterRuntime, WholeCharacterRender } from '../../entities/avatar/whole-character'
import type { WorldRendererPort, WorldRendererStatus } from '../composition'
import type { ReplicationView } from '../network/replication'
import type { AvatarSceneSprite, AvatarSceneTexture } from './scene'
import { canonicalJson } from '../map/json'
import { policy } from '../network/transport/contract'
import { sceneryDepth } from '../illustrated/model'

type Context = Parameters<WorldRendererPort['mount']>[0]
/** Root retains ownership. The adapter may release its leases, never this runtime. */
export type BorrowedWholeCharacterRuntime = Pick<WholeCharacterRuntime, 'scope' | 'isCurrent' | 'validateConfig' | 'acquire'>
const manifestHash = 'sha256:0627c7d178b22535a561e1270db6d8bf9e2383ba84d03a63962e48e565c59b2b'
const manifestId = 'mcr_selected_static_20261103'
/** Static front-idle art on the incumbent scene; no game, socket, prediction or animation loop. */
export function createWholeCharacterWorldRenderer(options: { resolveRuntime(context: Context): BorrowedWholeCharacterRuntime }) {
  const port: WorldRendererPort = { mount(context) {
    let disposed = false, status: WorldRendererStatus = 'loading', runtime: BorrowedWholeCharacterRuntime | null = null
    let latest: ReplicationView | null = null, baseline: string | null = null, sampling = false
    const scene = context.avatarScene, listeners = new Set<() => void>()
    type Asset = { key: string; config: unknown; controller: AbortController; refs: number; lease: WholeCharacterLease | null;
      unsubscribe: () => void; texture: AvatarSceneTexture | null; state: 'loading' | 'ready' | 'unavailable' }
    type Row = { id: string; key: string; revision: number; asset: Asset; sprite: AvatarSceneSprite | null; x: number; y: number; failed: boolean }
    const assets = new Map<string, Asset>(), rows = new Map<string, Row>()
    let removeFrame = () => {}
    const current = () => !disposed && !context.signal.aborted && !!runtime?.isCurrent()
    function notify(next: WorldRendererStatus) {
      if (disposed || status === next) return
      status = next
      for (const listener of Array.from(listeners)) { if (disposed) break; try { listener() } catch { /* Independent status observers. */ } }
    }
    function attempt(work: () => void) { try { work() } catch { /* Release all remaining owned resources. */ } }
    function release(asset: Asset) {
      if (assets.get(asset.key) !== asset) return
      assets.delete(asset.key); asset.controller.abort(); attempt(asset.unsubscribe)
      asset.unsubscribe = () => {}; const lease = asset.lease, texture = asset.texture; asset.lease = null; asset.texture = null
      attempt(() => texture?.dispose()); attempt(() => lease?.dispose())
    }
    function remove(row: Row) {
      if (rows.get(row.id) !== row) return
      rows.delete(row.id); attempt(() => row.sprite?.dispose()); row.sprite = null
      if (--row.asset.refs === 0) release(row.asset)
    }
    function clear() { for (const row of Array.from(rows.values())) remove(row); for (const asset of Array.from(assets.values())) release(asset) }
    function failAsset(asset: Asset) {
      if (assets.get(asset.key) !== asset) return
      if (!current()) { dispose(); return }
      asset.state = 'unavailable'
      // Revocation clears GPU allocations synchronously, before a later scene tick.
      for (const row of Array.from(rows.values())) if (row.asset === asset) { attempt(() => row.sprite?.dispose()); row.sprite = null; row.failed = true }
      const texture = asset.texture, lease = asset.lease; asset.texture = null; asset.lease = null
      attempt(asset.unsubscribe); asset.unsubscribe = () => {}; attempt(() => texture?.dispose()); attempt(() => lease?.dispose()); notify('unavailable')
    }
    function updateStatus() {
      if (!current()) { clear(); notify('unavailable'); return }
      notify(Array.from(rows.values()).some(row => row.failed || row.asset.state === 'unavailable') ? 'unavailable'
        : Array.from(assets.values()).some(asset => asset.state === 'loading') ? 'loading' : 'ready')
    }
    function validRender(render: WholeCharacterRender, asset: Asset) {
      const config = runtime!.validateConfig(render.config), index = Number(config.preset_id.slice(-2)) - 1
      return canonicalJson(config) === asset.key && render.pose === 'front_idle' && render.logicalCell.width === 64 && render.logicalCell.height === 64
        && render.anchor.x === 32 && render.anchor.y === 60 && render.source.x === index * 64 && render.source.y === 0
        && render.source.width === 64 && render.source.height === 64 && render.image.naturalWidth === 384 && render.image.naturalHeight === 64
    }
    function load(asset: Asset) {
      void Promise.resolve().then(() => {
        if (!current() || assets.get(asset.key) !== asset) return null
        return runtime!.acquire(asset.config, { signal: asset.controller.signal })
      }).then(lease => {
        if (!lease) return
        if (!current() || assets.get(asset.key) !== asset || asset.controller.signal.aborted) { lease.dispose(); return }
        asset.lease = lease
        const unsubscribe = lease.subscribe(() => { if (!lease.read()) failAsset(asset) })
        if (!current() || assets.get(asset.key) !== asset || asset.controller.signal.aborted) { unsubscribe(); lease.dispose(); return }
        asset.unsubscribe = unsubscribe
        const render = lease.read()
        if (!render || !validRender(render, asset) || asset.state === 'unavailable') throw new Error('Whole character unavailable')
        const cell = document.createElement('canvas'); cell.width = cell.height = 64
        let texture: AvatarSceneTexture
        try {
          const ctx = cell.getContext('2d'); if (!ctx) throw new Error('Whole character unavailable')
          ctx.imageSmoothingEnabled = false
          ctx.drawImage(render.image, render.source.x, 0, 64, 64, 0, 0, 64, 64)
          texture = scene!.staticTexture64!(cell)
        } finally { cell.width = cell.height = 0 }
        if (!current() || assets.get(asset.key) !== asset || !lease.read() || asset.controller.signal.aborted) { texture.dispose(); release(asset); return }
        asset.texture = texture; asset.state = 'ready'; paint(); updateStatus()
      }).catch(() => { if (assets.get(asset.key) === asset) { failAsset(asset); updateStatus() } })
    }
    function paint() {
      if (!current() || !scene) { clear(); notify('unavailable'); return }
      for (const row of Array.from(rows.values())) {
        if (!current()) { clear(); return }
        if (row.failed || row.asset.state !== 'ready' || !row.asset.texture) continue
        if (!row.asset.lease?.read()) { failAsset(row.asset); continue }
        try {
          if (!row.sprite) {
            const sprite = scene.sprite(row.asset.texture)
            if (!current() || rows.get(row.id) !== row) { sprite.dispose(); continue }
            row.sprite = sprite
          }
          row.sprite.update(row.x, row.y, 0, sceneryDepth(row.y, context.map.heightPx))
        } catch { attempt(() => row.sprite?.dispose()); row.sprite = null; row.failed = true }
      }
    }
    function replace(view: ReplicationView) {
      if (disposed) return
      if (!current() || !runtime || view.scope.userId !== runtime.scope.userId || view.scope.eventId !== runtime.scope.eventId
        || view.scope.generation !== runtime.scope.generation) { dispose(); return }
      latest = view
      if (!view.visible || !['live', 'switching'].includes(view.phase) || !view.baseline) { latest = null; clear(); baseline = null; notify('unavailable'); return }
      if (view.baseline.eventId !== context.release.resources.eventId || view.baseline.avatarManifestId !== manifestId || view.baseline.avatarManifestVersion !== 1) { latest = null; clear(); notify('unavailable'); return }
      const nextBaseline = JSON.stringify([view.baseline.streamId, view.baseline.roomEpoch, view.baseline.roomId])
      if (baseline !== nextBaseline) { clear(); baseline = nextBaseline }
      try {
        if (view.entities.length > policy.room_capacity_max || new Set(view.entities.map(e => e.player.userId)).size !== view.entities.length) throw new Error('Invalid roster')
        const wanted = view.entities.map(entity => {
          const config = runtime!.validateConfig(entity.player.avatar.config), { x, y } = entity.position, revision = entity.player.avatar.revision
          if (!Number.isSafeInteger(revision) || revision < 0 || !Number.isFinite(x) || !Number.isFinite(y) || x < 0 || y < 0
            || x > context.map.widthPx || y > context.map.heightPx) throw new Error('Invalid avatar')
          return { id: entity.player.userId, config, key: canonicalJson(config), revision, x, y }
        })
        const ids = new Set(wanted.map(row => row.id))
        for (const row of Array.from(rows.values())) {
          const next = wanted.find(next => next.id === row.id)
          if (!ids.has(row.id) || next!.key !== row.key || next!.revision !== row.revision) remove(row)
        }
        for (const next of wanted) {
          if (!current()) { clear(); return }
          const old = rows.get(next.id)
          if (old) { old.x = next.x; old.y = next.y; continue }
          let asset = assets.get(next.key)
          if (!asset) {
            asset = { key: next.key, config: next.config, controller: new AbortController(), refs: 0, lease: null, unsubscribe: () => {}, texture: null, state: 'loading' }
            assets.set(next.key, asset); load(asset)
          }
          asset.refs++; rows.set(next.id, { ...next, asset, sprite: null, failed: false })
        }
        if (!sampling) paint(); updateStatus()
      } catch { latest = null; clear(); notify('unavailable') }
    }
    function dispose() {
      if (disposed) return
      disposed = true; context.signal.removeEventListener('abort', dispose); attempt(removeFrame); clear(); latest = null; listeners.clear()
    }
    context.signal.addEventListener('abort', dispose, { once: true })
    try {
      if (context.signal.aborted || !scene?.staticTexture64) throw new Error('Static scene unavailable')
      const manifest = context.release.avatarManifest
      if (manifest.representation !== 'whole_character' || manifest.manifest_id !== manifestId || manifest.manifest_version !== 1 || manifest.content_hash !== manifestHash) throw new Error('Whole character release unavailable')
      runtime = options.resolveRuntime(context)
      if (!current() || runtime.scope.gate !== 'open' || runtime.scope.eventId !== context.release.resources.eventId) throw new Error('Whole character scope unavailable')
      const unsubscribe = scene.onFrame(() => {
        if (!current()) { dispose(); return }
        sampling = true
        try { const view = context.sample(); if (view) replace(view); else { latest = null; clear(); notify('unavailable') } }
        catch { latest = null; clear(); notify('unavailable') }
        finally { sampling = false }
        if (latest) { paint(); updateStatus() }
      })
      if (disposed) unsubscribe(); else removeFrame = unsubscribe
      notify('ready')
    } catch { latest = null; clear(); notify('unavailable'); if (context.signal.aborted) dispose() }
    return { replace, dispose, readStatus: () => disposed ? 'unavailable' : status,
      subscribeStatus(listener) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } } }
  } }
  return port
}
