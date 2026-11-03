import { build } from 'vite'
import react from '@vitejs/plugin-react'
import { rm } from 'node:fs/promises'
const owned = 'src/features/auth/__tests__/reference-target/.cache'
try { await build({ configFile: false, plugins: [react()], cacheDir: `${owned}/vite-build`, build: { outDir: `${owned}/build`, copyPublicDir: false, rollupOptions: { input: 'src/features/auth/__tests__/reference-target/preview.html' } } }) }
finally { await rm(owned, { recursive: true, force: true }) }
