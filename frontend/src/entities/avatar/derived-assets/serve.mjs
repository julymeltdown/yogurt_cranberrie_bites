import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../../../../../', import.meta.url))
const base = '/src/entities/avatar/derived-assets/'
const routes = new Map([
  [`${base}review.html`, ['frontend/src/entities/avatar/derived-assets/review.html', 'text/html']],
  [`${base}source-layout.json`, ['frontend/src/entities/avatar/derived-assets/source-layout.json', 'application/json']],
  [`${base}hair-review.html`, ['frontend/src/entities/avatar/derived-assets/hair-review.html', 'text/html']],
  [`${base}hair-layout.json`, ['frontend/src/entities/avatar/derived-assets/hair-layout.json', 'application/json']],
  ['/world/avatars/supplied-v1/photo-01.jpg', ['frontend/public/world/avatars/supplied-v1/photo-01.jpg', 'image/jpeg']],
  ['/world/avatars/supplied-v1/photo-04.jpg', ['frontend/public/world/avatars/supplied-v1/photo-04.jpg', 'image/jpeg']],
  ['/world/avatars/supplied-v1/photo-05.jpg', ['frontend/public/world/avatars/supplied-v1/photo-05.jpg', 'image/jpeg']],
  ['/evidence/rejected.png', ['docs/mcr/art/evidence/rejected/imagegen-photo01-r1.png', 'image/png']],
])

/** Small isolated evidence server; never mounts the application or arbitrary filesystem paths. */
export function createArtReviewServer() {
  return createServer(async (request, response) => {
    const route = routes.get(new URL(request.url, 'http://127.0.0.1').pathname)
    if (!route || request.method !== 'GET') { response.writeHead(404); response.end(); return }
    try {
      const bytes = await readFile(`${root}${route[0]}`)
      response.writeHead(200, { 'Content-Type': route[1], 'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff' })
      response.end(bytes)
    } catch { response.writeHead(500); response.end('Review resource unavailable') }
  })
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const server = createArtReviewServer()
  server.listen(Number(process.env.MCR_ART_PORT ?? 4193), '127.0.0.1', () => console.log(`Review: http://127.0.0.1:${server.address().port}${base}review.html`))
}
