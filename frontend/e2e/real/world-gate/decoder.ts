import { createServer } from 'vite'
import { fileURLToPath } from 'node:url'
import type { Decoder } from './observer'
/** Node 24 requires JSON import attributes; the app uses Vite JSON modules.
 * Load its exact committed C05 decoder through a non-listening module runner.
 * No browser requests, product resources, service mocks or source edits are involved.
 */
export async function loadDecoder(): Promise<Decoder> {
  const loader = await createServer({ configFile: false, root: fileURLToPath(new URL('../../../', import.meta.url)),
    server: { middlewareMode: true, watch: null, ws: false }, logLevel: 'silent' })
  try {
    const module = await loader.ssrLoadModule('/src/world/network/transport/contract.ts') as { decode: Decoder }
    return module.decode
  } finally { await loader.close() }
}
