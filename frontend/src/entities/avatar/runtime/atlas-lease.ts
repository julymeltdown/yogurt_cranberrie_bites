import { canonicalJson } from '../../../world/map/json'
import { abortable, type AssetResources } from '../../../world/map/resources'
import { AvatarError, ensure, validateManifest, type AvatarManifest } from './contract'
import { loadAtlas, validateBinding, type ReviewedAvatarBinding } from './resources'

export interface ReviewedAtlasInput {
  readonly document: unknown
  readonly pin: { manifest_id: string; manifest_version: number; content_hash: string }
  readonly binding?: ReviewedAvatarBinding
  readonly resources: AssetResources
}
type Atlas = Awaited<ReturnType<typeof loadAtlas>>
type Policy = ReturnType<typeof validateBinding>
type Entry = { manifest: AvatarManifest; policy: Policy; bytes: number; refs: number; pending: number;
  flight?: Promise<Atlas>; atlas?: Atlas }

/** Explicit owner-local decoded image cache. It never stores private outfit keys or DOM canvases.
 * isCurrent must check the owner's original user/event/session/privacy epoch, not recapture one.
 */
export function createReviewedAtlasLeases(options: { isCurrent(): boolean; maxEntries?: number; maxDecodedBytes?: number }) {
  const maximum = options.maxEntries ?? 2, budget = options.maxDecodedBytes ?? 32 * 1024 * 1024
  ensure(Number.isSafeInteger(maximum) && maximum > 0 && maximum <= 4
    && Number.isSafeInteger(budget) && budget > 0 && budget <= 64 * 1024 * 1024)
  const lifetime = new AbortController(), entries = new Map<string, Entry>(), identities = new WeakMap<AssetResources, number>()
  let nextIdentity = 0, bytes = 0, loads = 0, hits = 0, disposed = false
  const remove = (key: string, entry: Entry) => {
    if (entries.get(key) !== entry) return
    entries.delete(key); bytes -= entry.bytes; entry.atlas?.dispose()
  }
  const dispose = () => {
    if (disposed) return
    disposed = true; lifetime.abort()
    for (const [key, entry] of entries) remove(key, entry)
  }
  const current = () => {
    if (!options.isCurrent()) dispose()
    if (disposed) throw new AvatarError('cancelled')
  }
  return Object.freeze({
    signal: lifetime.signal,
    async acquire(input: ReviewedAtlasInput, signal: AbortSignal) {
      current(); if (signal.aborted) throw new AvatarError('cancelled')
      // Snapshot mutable caller input before awaiting hash validation. Recheck receipts on every hit.
      const text = canonicalJson(input.document), pin = { ...input.pin }, resources = input.resources
      const binding = input.binding ? JSON.parse(canonicalJson(input.binding)) as ReviewedAvatarBinding : undefined
      const manifest = await validateManifest(text, pin), policy = validateBinding(manifest, binding)
      current(); if (signal.aborted) throw new AvatarError('cancelled')
      let identity = identities.get(resources)
      if (identity === undefined) { identity = ++nextIdentity; identities.set(resources, identity) }
      const key = canonicalJson([identity, pin, binding]), size = manifest.atlas.texture.width_px * manifest.atlas.texture.height_px * 4
      let entry = entries.get(key)
      if (!entry) {
        if (size > budget) throw new AvatarError('capacity')
        while (entries.size >= maximum || bytes + size > budget) {
          const victim = [...entries].find(([, value]) => value.atlas && value.refs === 0 && value.pending === 0)
          if (!victim) throw new AvatarError('capacity')
          remove(...victim)
        }
        const candidate: Entry = { manifest, policy, bytes: size, refs: 0, pending: 0 }
        entries.set(key, candidate); bytes += size; loads++
        candidate.flight = loadAtlas(manifest, resources, lifetime.signal).then(atlas => {
          if (disposed || !options.isCurrent() || entries.get(key) !== candidate) {
            atlas.dispose(); current(); throw new AvatarError('cancelled')
          }
          candidate.atlas = atlas; return atlas
        }).catch(error => { remove(key, candidate); throw error })
        entry = candidate
      } else { hits++; entries.delete(key); entries.set(key, entry) }
      const retained = entry
      retained.pending++
      try {
        const atlas = await abortable(retained.flight!, AbortSignal.any([signal, lifetime.signal]))
        current(); if (signal.aborted) throw new AvatarError('cancelled')
        retained.refs++
        let released = false
        const release = () => {
          if (released) return
          released = true; signal.removeEventListener('abort', release); retained.refs--
        }
        signal.addEventListener('abort', release, { once: true })
        return { manifest: retained.manifest, ...retained.policy, image: atlas.image, dispose: release }
      } finally { retained.pending-- }
    },
    read: () => ({ entries: entries.size, reservedDecodedBytes: bytes, references: [...entries.values()].reduce((n, e) => n + e.refs, 0),
      pending: [...entries.values()].reduce((n, e) => n + e.pending, 0), loads, hits, disposed }),
    dispose,
  })
}
export type ReviewedAtlasLeases = ReturnType<typeof createReviewedAtlasLeases>
