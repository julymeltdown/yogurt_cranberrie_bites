import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig, type Plugin } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

// The venue root binding test consumes the supplied art through one virtual module.
// Read the immutable public files at load time so the test never copies or rewrites them.
const venueArt = fileURLToPath(new URL('./public/venue/paradise-city/v1/', import.meta.url));
function venueRootArt(): Plugin {
  const source = 'virtual:venue-root-art';
  const resolved = '\0' + source;
  return {
    name: 'mcr-venue-root-art',
    resolveId(id) { if (id === source) return resolved; },
    load(id) {
      if (id !== resolved) return;
      const manifestText = readFileSync(venueArt + 'asset-interface.json', 'utf8');
      const imageBase64 = readFileSync(venueArt + 'siteplan-photo-5.jpg').toString('base64');
      return `export const manifestText = ${JSON.stringify(manifestText)};\nexport const imageBase64 = ${JSON.stringify(imageBase64)};\n`;
    },
  };
}

export default defineConfig({
  plugins: [react(), tailwindcss(), venueRootArt()],
  server: { host: '127.0.0.1', port: 5173, strictPort: true },
  preview: { host: '127.0.0.1', port: 4173, strictPort: true },
  build: { target: 'es2022', sourcemap: false },
});
