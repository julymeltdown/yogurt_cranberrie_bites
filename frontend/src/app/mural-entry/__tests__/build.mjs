import { build } from 'vite'

// The default registry is reserved/unmodified. Also production-build the new adapter entry itself.
await build({ configLoader: 'runner', cacheDir: '/tmp/mcr-fe33-entry-build-cache', build: {
  outDir: '/tmp/mcr-fe33-entry-adapter-build', emptyOutDir: true, sourcemap: false,
  lib: { entry: new URL('../index.ts', import.meta.url).pathname, formats: ['es'], fileName: 'mural-entry' },
  rolldownOptions: { external: ['react', 'react-dom', 'react/jsx-runtime', 'react-router-dom'] },
} })
