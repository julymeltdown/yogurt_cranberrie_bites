import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { mkdtemp, rm } from 'node:fs/promises'
import { aliases } from './vitest.config.mjs'
const dependencies = process.env.MCR_FRONTEND_DEPENDENCIES
if (!dependencies) throw Error('Set MCR_FRONTEND_DEPENDENCIES')
const require = createRequire(resolve(dependencies, 'package.json')), { build } = require('vite')
const root = resolve(import.meta.dirname, '../../../../..'), temporary = await mkdtemp('/private/tmp/mcr-motion-build-')
const application = process.argv.includes('--application')
try {
  const result = await build({ configFile: false, root, logLevel: 'error', cacheDir: resolve(temporary, 'cache'),
    ...(application ? { plugins: [require('@vitejs/plugin-react').default(), ...require('@tailwindcss/vite').default()] } : {}),
    resolve: { alias: Object.entries(aliases).sort(([a], [b]) => b.length - a.length).map(([find, replacement]) => ({ find, replacement })) },
    build: { target: 'es2022', outDir: resolve(temporary, 'dist'), copyPublicDir: false,
      rollupOptions: { input: resolve(root, application ? 'index.html' : 'src/app/character-entry/__tests__/motion-adoption/fixture.html') } },
  })
  console.log(JSON.stringify({ result: 'PASS', qualification: application ? 'Optimized actual index.html/main.tsx application build with retained React/Tailwind plugins. Positive v2 Root/runtime admission remains NOT_RUN.'
    : 'Optimized new consumer/Phaser fixture build; not whole-application production admission.', outputs: Array.isArray(result) ? result.length : 1 }))
} finally { await rm(temporary, { recursive: true, force: true }) }
