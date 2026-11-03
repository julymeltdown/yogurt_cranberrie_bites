import assert from 'node:assert/strict'
import { statfs, rm } from 'node:fs/promises'
import { build } from 'vite'
import react from '@vitejs/plugin-react'
const own='src/features/me/__tests__/reference-target/.cache',fs=await statfs('.')
assert(fs.bavail*fs.bsize>8*1024**3+100*1024**2,'disk needs >8GiB plus100MiB headroom')
try {await build({configFile:false,plugins:[react()],cacheDir:`${own}/vite-build`,build:{outDir:`${own}/build`,copyPublicDir:false,rollupOptions:{input:'src/features/me/__tests__/reference-target/preview.html'}}})}
finally {await rm(`${own}/build`,{recursive:true,force:true});await rm(`${own}/vite-build`,{recursive:true,force:true})}
