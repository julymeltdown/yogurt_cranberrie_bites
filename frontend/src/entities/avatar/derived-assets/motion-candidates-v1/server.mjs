import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
export const root = fileURLToPath(new URL('../../../../../../', import.meta.url))
export const toolsPath = 'frontend/src/entities/avatar/derived-assets/motion-candidates-v1/'
export const publicPath = 'frontend/public/world/avatars/derived-v1/motion-candidates-v1/'
export const evidencePath = 'docs/mcr/art/evidence/motion-candidates-v1/'
/** Isolated candidate fixture only; no product descriptor/registry or credentials. */
export function candidateServer() {
  return createServer(async (req, res) => {
    const p = new URL(req.url, 'http://localhost').pathname
    const f = p.startsWith('/tools/') && /^[a-z-]+\.(mjs|html|json)$/.test(p.slice(7)) ? toolsPath + p.slice(7)
      : /^\/world\/avatars\/(supplied-v1|derived-v1\/motion-candidates-v1)\/[a-z0-9.-]+\.(jpg|png|json|bin)$/.test(p) ? 'frontend/public' + p
        : /^\/world\/themes\/(revenge|parade|danger)\.png$/.test(p) ? 'frontend/public' + p : null
    if (!f || req.method !== 'GET') { res.writeHead(404); res.end(); return }
    try { const bytes = await readFile(root + f); res.writeHead(200, { 'Content-Type': f.endsWith('.mjs') ? 'text/javascript' : f.endsWith('.html') ? 'text/html' : f.endsWith('.json') ? 'application/json' : f.endsWith('.png') ? 'image/png' : f.endsWith('.bin') ? 'application/octet-stream' : 'image/jpeg', 'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff' }); res.end(bytes) }
    catch { res.writeHead(404); res.end() }
  })
}
