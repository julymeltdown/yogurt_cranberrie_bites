import { defineConfig, mergeConfig } from 'vite'
import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import base from '../../../../vite.config.ts'
const cache = process.env.MCR_FEED_CACHE
if (!cache?.startsWith('/')) throw Error('Use the accepted private Feed allocation cache')
const cssFile = process.env.MCR_FEED_CSS_FILE, cssHash = process.env.MCR_FEED_CSS_SHA256
if (Boolean(cssFile) !== Boolean(cssHash)) throw Error('Supply both immutable CSS input pins')
const css = cssFile ? readFileSync(cssFile, 'utf8') : null
if (css !== null && createHash('sha256').update(css).digest('hex') !== cssHash) throw Error('CSS dependency pin mismatch')
export default mergeConfig(base, defineConfig({ cacheDir: cache + '/vite', plugins: css === null ? [] : [{
  name: 'explicit-feed-owner-css-input', enforce: 'pre',
  load(id) { return id.endsWith('/src/features/event-feed/feed.css') ? css : null },
}] }))
