import { build } from 'vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'node:path'
import { rm } from 'node:fs/promises'
const frontend = resolve(import.meta.dirname, '../../../../..'), scratch = resolve(import.meta.dirname, '.scratch-cache/build')
try {
  await build({ configFile: false, plugins: [react()], root: frontend, cacheDir: resolve(scratch, 'cache'),
    build: { target: 'es2022', outDir: resolve(scratch, 'dist'), emptyOutDir: true,
      rollupOptions: { input: resolve(import.meta.dirname, 'fixtures/preview.html') } },
  })
} finally { await rm(scratch, { recursive: true, force: true }) }
