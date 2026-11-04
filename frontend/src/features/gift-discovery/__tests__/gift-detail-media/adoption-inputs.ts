import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { fileURLToPath } from 'node:url'

// Root-authorized immutable read-only inputs, loaded at their original module IDs.
// No shared file or Root checkout is mutated by this qualification.
const entries = [
  ['src/app/CommunityFeatures.tsx', 'CommunityFeatures.tsx.source', 'db2b1d0330b5af0347e55fda017da4d0bd1b586e9c2e8872d8fbae1390e577d8'],
  ['src/app/media-entry/RootMediaView.tsx', 'RootMediaView.tsx.source', 'f22e034423804ec41658fa26760d07926f4f2ebce339fed685072fbf09542d9a'],
] as const
export function adoptionSources() {
  return new Map(entries.map(([id, file, sha]) => {
    const source = readFileSync(new URL(`./adoption-inputs/${file}`, import.meta.url), 'utf8')
    if (createHash('sha256').update(source).digest('hex') !== sha) throw Error(`Root input changed: ${id}`)
    return [fileURLToPath(new URL(`../../../../../${id}`, import.meta.url)), source]
  }))
}
export function adoptionPlugin() {
  const sources = adoptionSources()
  return { name: 'root-authorized-gifts-adoption', enforce: 'pre' as const,
    load(id: string) { return sources.get(id.split('?')[0]!) ?? null } }
}
