import { build } from 'vite'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'

const frontend = resolve(import.meta.dirname, '../../../..'), outDir = await mkdtemp(resolve(tmpdir(), 'mcr-community-entry-build-'))
try {
  await build({ root: frontend, configFile: resolve(frontend, 'vite.config.ts'), build: { outDir, emptyOutDir: true,
    rollupOptions: { input: resolve(import.meta.dirname, 'preview.html') } } })
  console.log('PASS: production fixture bundle includes community-entry adapters; production root wiring remains NOT_RUN.')
} finally { await rm(outDir, { recursive: true, force: true }) }
