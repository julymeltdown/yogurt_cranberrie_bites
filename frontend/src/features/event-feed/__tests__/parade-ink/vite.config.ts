import { defineConfig, mergeConfig } from 'vite'
import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { dirname, isAbsolute, resolve } from 'node:path'
import base from '../../../../../vite.config.ts'

const temporary = process.env.MCR_PARADE_FEED_TMP
const manifest = process.env.MCR_PI_FOUNDATION_INPUTS
if (!temporary?.startsWith('/private/tmp/mcr-parade-ink-feed-thread-') || !manifest) throw Error('Accepted private PI02/03 native allocation required')
const frontend = resolve(import.meta.dirname, '../../../../..')
const pins = JSON.parse(readFileSync(manifest, 'utf8')) as { path: string; file: string; sha256: string }[]
const entries = new Map(pins.map(pin => {
  if (!pin.path.startsWith('frontend/src/') || !isAbsolute(pin.file) || !pin.file.startsWith(temporary + '/')) throw Error('Invalid immutable foundation input')
  const bytes = readFileSync(pin.file)
  if (createHash('sha256').update(bytes).digest('hex') !== pin.sha256) throw Error('Foundation postimage pin mismatch')
  return [resolve(frontend, pin.path.slice('frontend/'.length)), { ...pin, bytes }] as const
}))
function css(path: string, stack: readonly string[] = []): string {
  if (stack.includes(path)) throw Error('Foundation CSS import cycle')
  const body = entries.get(path)!.bytes.toString('utf8')
  return body.replace(/@import\s+["']([^"']+)["'];/g, (original, relative: string) => {
    const imported = resolve(dirname(path), relative)
    return entries.has(imported) ? css(imported, [...stack, path]) : original
  }).replace(/url\(["']?(\.[^)'"\s]+)["']?\)/g, (original, relative: string) => {
    const asset = entries.get(resolve(dirname(path), relative))
    return asset ? `url("/@fs/${asset.file}")` : original
  })
}
export default mergeConfig(base, defineConfig({
  cacheDir: `${temporary}/vite`,
  define: { __PI_CANDIDATE__: JSON.stringify(process.env.MCR_PI_CANDIDATE), __PI_FOUNDATION_PINS__: JSON.stringify(pins.map(({ path, sha256 }) => ({ path, sha256 }))) },
  server: { fs: { allow: [frontend, temporary] } },
  plugins: [{ name: 'immutable-PI01-default-consumer-inputs', enforce: 'pre',
    resolveId(source, importer) {
      const path = source.startsWith('.') && importer ? resolve(dirname(importer.split('?')[0]!), source) : source
      return entries.has(path) ? path : null
    },
    load(id) {
      const path = id.split('?')[0]!, entry = entries.get(path)
      return entry ? path.endsWith('.css') ? css(path) : entry.bytes.toString('utf8') : null
    },
  }],
}))
