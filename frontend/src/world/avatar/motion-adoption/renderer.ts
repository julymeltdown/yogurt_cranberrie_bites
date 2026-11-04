import type { MotionLease, MotionRuntime } from '../../../entities/avatar/runtime/motion-adoption/runtime'
import { motionFrame } from '../../../entities/avatar/runtime/motion-adoption/art'
import { canonicalJson } from '../../map/json'
import { isReviewedProductionAuthority } from '../../../entities/avatar/runtime/motion-adoption/reviewed-production'
import type { WorldRendererPort, WorldRendererStatus } from '../../composition'
import type { ReplicationView } from '../../network/replication'
import type { AvatarSceneSprite, AvatarSceneTexture } from '../scene'
import { sceneryDepth } from '../../illustrated/model'

type Context = Parameters<WorldRendererPort['mount']>[0]
/** One incumbent scene clock. Production requires the factory-minted current authority; fixtures remain explicitly TEST_ONLY. */
export function createMotionWorldRenderer<C>(options: {
  readonly qualification: 'TEST_ONLY' | 'REVIEWED_PRODUCTION'
  resolveRuntime(context: Context): MotionRuntime<C>
}) {
  return { mount(context: Context) {
    let disposed = false, status: WorldRendererStatus = 'loading', runtime: MotionRuntime<C> | null = null
    const scene = context.avatarScene, listeners = new Set<() => void>()
    let baseline: string | null = null, removeFrame = () => {}, time = 0
    type Asset = { key: string; config: C; refs: number; controller: AbortController; lease: MotionLease<C> | null;
      texture: AvatarSceneTexture | null; unsubscribe(): void; failed: boolean }
    type Row = { id: string; key: string; revision: number; asset: Asset; sprite: AvatarSceneSprite | null;
      x: number; y: number; facing: 'down' | 'left' | 'right' | 'up'; moving: boolean }
    const assets = new Map<string, Asset>(), rows = new Map<string, Row>()
    const current = () => !disposed && !context.signal.aborted && !!runtime?.isCurrent()
    function notify(next: WorldRendererStatus) { if (disposed || status === next) return; status = next; for (const fn of Array.from(listeners)) { try { fn() } catch { /* Independent observer. */ } } }
    const attempt = (work: () => void) => { try { work() } catch { /* Complete independent cleanup. */ } }
    function release(asset: Asset) {
      if (assets.get(asset.key) !== asset) return
      assets.delete(asset.key); asset.controller.abort(); attempt(asset.unsubscribe)
      const texture = asset.texture, lease = asset.lease; asset.texture = null; asset.lease = null
      attempt(() => texture?.dispose()); attempt(() => lease?.dispose())
    }
    function remove(row: Row) { rows.delete(row.id); attempt(() => row.sprite?.dispose()); row.sprite = null; if (--row.asset.refs === 0) release(row.asset) }
    function clear() { for (const row of Array.from(rows.values())) remove(row); for (const asset of Array.from(assets.values())) release(asset) }
    function fail(asset: Asset) {
      if (assets.get(asset.key) !== asset) return
      asset.failed = true
      for (const row of rows.values()) if (row.asset === asset) { attempt(() => row.sprite?.dispose()); row.sprite = null }
      const texture = asset.texture; asset.texture = null; attempt(() => texture?.dispose()); notify('unavailable')
      if (!current()) dispose()
    }
    function paint() {
      if (!current() || !scene) { clear(); return }
      let pending = false, failed = false
      for (const row of rows.values()) {
        if (row.asset.failed) { failed = true; continue }
        if (!row.asset.texture) { pending = true; continue }
        if (!row.asset.lease?.read()) { fail(row.asset); failed = true; continue }
        try {
          row.sprite ??= scene.sprite(row.asset.texture)
          row.sprite.update(row.x, row.y, motionFrame(row.facing, row.moving, time), sceneryDepth(row.y, context.map.heightPx))
        } catch { fail(row.asset); failed = true }
      }
      notify(failed ? 'unavailable' : pending ? 'loading' : 'ready')
    }
    function load(asset: Asset) {
      void Promise.resolve().then(() => current() ? runtime!.acquire(asset.config, { signal: asset.controller.signal }) : null).then(lease => {
        if (!lease) return
        if (!current() || assets.get(asset.key) !== asset || asset.controller.signal.aborted) { lease.dispose(); return }
        asset.lease = lease; asset.unsubscribe = lease.subscribe(() => { if (!lease.read()) fail(asset) })
        const render = lease.read()
        if (!render || canonicalJson(render.config as Record<string, unknown>) !== asset.key || render.canvas.width !== 320 || render.canvas.height !== 256) throw Error('Invalid motion composition')
        const texture = scene!.texture(render.canvas)
        if (!current() || assets.get(asset.key) !== asset || !lease.read()) { texture.dispose(); release(asset); return }
        asset.texture = texture; paint()
      }).catch(() => { if (assets.get(asset.key) === asset) fail(asset) })
    }
    function replace(view: ReplicationView) {
      if (disposed) return
      const scope = runtime?.scope
      if (!current() || !scope || view.scope.userId !== scope.userId || view.scope.eventId !== scope.eventId || view.scope.generation !== scope.generation) { dispose(); return }
      if (!view.visible || !['live', 'switching'].includes(view.phase) || !view.baseline) { clear(); baseline = null; notify('unavailable'); return }
      try {
        const b = view.baseline, authority = runtime!.authority
        if (b.eventId !== scope.eventId || b.avatarManifestId !== authority.manifestId || b.avatarManifestVersion !== authority.manifestVersion
          || view.entities.length > 50 || new Set(view.entities.map(e => e.player.userId)).size !== view.entities.length) throw Error('Invalid motion roster')
        const wanted = view.entities.map(({ player, position }) => {
          const config = runtime!.validateConfig(player.avatar.config), { x, y } = position, revision = player.avatar.revision
          if (!Number.isSafeInteger(revision) || revision < 1 || !Number.isFinite(x) || !Number.isFinite(y) || x < 0 || y < 0
            || x > context.map.widthPx || y > context.map.heightPx) throw Error('Invalid motion position')
          motionFrame(player.facing, player.moving, time)
          return { id: player.userId, key: canonicalJson(config as Record<string, unknown>), config, revision, x, y, facing: player.facing, moving: player.moving }
        })
        const nextBaseline = JSON.stringify([b.streamId, b.roomEpoch, b.roomId])
        if (baseline !== nextBaseline) { clear(); baseline = nextBaseline }
        const wantedById = new Map(wanted.map(row => [row.id, row]))
        for (const row of Array.from(rows.values())) {
          const next = wantedById.get(row.id)
          if (!next || next.key !== row.key || next.revision !== row.revision) remove(row)
        }
        for (const next of wanted) {
          const old = rows.get(next.id)
          if (old) { Object.assign(old, { x: next.x, y: next.y, facing: next.facing, moving: next.moving }); continue }
          let asset = assets.get(next.key)
          if (!asset) { asset = { key: next.key, config: next.config, refs: 0, controller: new AbortController(), lease: null,
            texture: null, unsubscribe() {}, failed: false }; assets.set(next.key, asset); load(asset) }
          asset.refs++; rows.set(next.id, { ...next, asset, sprite: null })
        }
        paint()
      } catch { clear(); notify('unavailable') }
    }
    function dispose() { if (disposed) return; disposed = true; context.signal.removeEventListener('abort', dispose); attempt(removeFrame); clear(); listeners.clear() }
    context.signal.addEventListener('abort', dispose, { once: true })
    try {
      if (context.signal.aborted || !scene || !['TEST_ONLY', 'REVIEWED_PRODUCTION'].includes(options.qualification)) throw Error('Motion scene unavailable')
      runtime = options.resolveRuntime(context)
      const manifest = context.release.avatarManifest, authority = runtime.authority
      if (!current() || authority.qualification !== options.qualification
        || options.qualification === 'REVIEWED_PRODUCTION' && !isReviewedProductionAuthority(authority)
        || manifest.manifest_id !== authority.manifestId
        || manifest.manifest_version !== authority.manifestVersion || manifest.content_hash !== authority.manifestHash) throw Error('Motion authority unavailable')
      removeFrame = scene.onFrame(next => {
        if (!current()) { dispose(); return }
        time = next; runtime!.sampleFrame(next)
        try { const view = context.sample(); if (view) replace(view); else { clear(); notify('unavailable') } }
        catch { clear(); notify('unavailable') }
      })
      notify('ready')
    } catch { clear(); notify('unavailable') }
    return { replace, dispose, readStatus: () => disposed ? 'unavailable' as const : status,
      subscribeStatus(listener: () => void) { if (disposed) return () => {}; listeners.add(listener); return () => { listeners.delete(listener) } } }
  } } satisfies WorldRendererPort
}
