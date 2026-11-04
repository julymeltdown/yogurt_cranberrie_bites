import { build } from 'vite'
import react from '@vitejs/plugin-react'
import { rm } from 'node:fs/promises'
const cache = 'src/features/auth/__tests__/reference-target/fidelity/.build-cache'
try { await build({ configFile: false, plugins: [react()], cacheDir: `${cache}/vite`, build: { outDir: `${cache}/output`, copyPublicDir: false, rollupOptions: { input: 'src/features/auth/__tests__/reference-target/fidelity/app.html' } } }) }
finally { await rm(cache, { recursive: true, force: true }) }
