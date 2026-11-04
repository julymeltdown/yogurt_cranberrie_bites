import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { mkdtemp, rm } from 'node:fs/promises'
import { aliases } from './vitest.config.mjs'
const dependencies = process.env.MCR_FRONTEND_DEPENDENCIES
if (!dependencies) throw Error('Set MCR_FRONTEND_DEPENDENCIES')
const require = createRequire(resolve(dependencies, 'package.json')), { build } = require('vite')
const root = resolve(import.meta.dirname, '../../../../..'), temporary = await mkdtemp('/private/tmp/mcr-motion-build-')
try {
  const result = await build({ configFile: false, root, logLevel: 'error', cacheDir: resolve(temporary, 'cache'),
    resolve: { alias: Object.entries(aliases).sort(([a], [b]) => b.length - a.length).map(([find, replacement]) => ({ find, replacement })) },
    build: { target: 'es2022', outDir: resolve(temporary, 'dist'), copyPublicDir: false,
      rollupOptions: { input: resolve(root, 'src/app/character-entry/__tests__/motion-adoption/fixture.html') } },
  })
  console.log(JSON.stringify({ result: 'PASS', qualification: 'Optimized new consumer/Phaser fixture build; not whole-application production admission.', outputs: Array.isArray(result) ? result.length : 1 }))
} finally { await rm(temporary, { recursive: true, force: true }) }
