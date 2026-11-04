import { defineConfig, mergeConfig } from 'vite'
import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import base from '../../../../vite.config.ts'
const cache = process.env.MCR_FEED_CACHE
if (!cache?.startsWith('/')) throw Error('Use the accepted private Feed allocation cache')
const inputs = [
  { suffix: '/src/features/event-feed/feed.css', file: process.env.MCR_FEED_CSS_FILE, hash: process.env.MCR_FEED_CSS_SHA256 },
  { suffix: '/src/shared/styles/cozy-community-backgrounds-v1.css', file: process.env.MCR_FEED_ROOT_CSS_FILE, hash: process.env.MCR_FEED_ROOT_CSS_SHA256 },
  { suffix: '/src/app/media-entry/RootMediaView.tsx', file: process.env.MCR_FEED_ROOT_MEDIA_FILE, hash: process.env.MCR_FEED_ROOT_MEDIA_SHA256 },
  { suffix: '/src/app/social-composition/panels.tsx', file: process.env.MCR_FEED_ROOT_SOCIAL_FILE, hash: process.env.MCR_FEED_ROOT_SOCIAL_SHA256 },
].flatMap(input => {
  if (Boolean(input.file) !== Boolean(input.hash)) throw Error('Supply both immutable owner input pins')
  if (!input.file) return []
  const css = readFileSync(input.file, 'utf8')
  if (createHash('sha256').update(css).digest('hex') !== input.hash) throw Error('Owner dependency pin mismatch')
  return [{ suffix: input.suffix, css }]
})
export default mergeConfig(base, defineConfig({ cacheDir: cache + '/vite', plugins: inputs.length === 0 ? [] : [{
  name: 'explicit-feed-and-root-owner-inputs', enforce: 'pre',
  load(id) {
    const path = id.split('?')[0]!
    const root = inputs.find(input => input.suffix.endsWith('/cozy-community-backgrounds-v1.css'))
    // PostCSS resolves nested @import files directly; inline this exact pinned
    // dependency into the served parent without changing either source file.
    if (root && path.endsWith('/src/shared/styles/base.css')) return readFileSync(path, 'utf8')
      .replace('@import "./cozy-community-backgrounds-v1.css";', root.css)
    return inputs.find(input => path.endsWith(input.suffix))?.css ?? null
  },
}] }))
