import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { createHash } from 'node:crypto'

const root = fileURLToPath(new URL('../../../../../', import.meta.url))
const base = '/src/entities/avatar/derived-assets/'
const routes = new Map([
  [`${base}review.html`, ['frontend/src/entities/avatar/derived-assets/review.html', 'text/html']],
  [`${base}source-layout.json`, ['frontend/src/entities/avatar/derived-assets/source-layout.json', 'application/json']],
  [`${base}hair-review.html`, ['frontend/src/entities/avatar/derived-assets/hair-review.html', 'text/html']],
  [`${base}hair-layout.json`, ['frontend/src/entities/avatar/derived-assets/hair-layout.json', 'application/json']],
  [`${base}mask-presets.mjs`, ['frontend/src/entities/avatar/derived-assets/mask-presets.mjs', 'text/javascript']],
  [`${base}static-review.html`, ['frontend/src/entities/avatar/derived-assets/static-review.html', 'text/html']],
  [`${base}static-package.json`, ['frontend/src/entities/avatar/derived-assets/static-package.json', 'application/json']],
  ['/world/avatars/supplied-v1/photo-01.jpg', ['frontend/public/world/avatars/supplied-v1/photo-01.jpg', 'image/jpeg']],
  ['/world/avatars/supplied-v1/photo-04.jpg', ['frontend/public/world/avatars/supplied-v1/photo-04.jpg', 'image/jpeg']],
  ['/world/avatars/supplied-v1/photo-05.jpg', ['frontend/public/world/avatars/supplied-v1/photo-05.jpg', 'image/jpeg']],
  ['/evidence/rejected.png', ['docs/mcr/art/evidence/rejected/imagegen-photo01-r1.png', 'image/png']],
])

/** Small isolated evidence server; never mounts the application or arbitrary filesystem paths. */
export function createArtReviewServer() {
  return createServer(async (request, response) => {
    const pathname=new URL(request.url, 'http://127.0.0.1').pathname
    const derived=/^\/world\/avatars\/derived-v1\/([a-z0-9_.-]+\.(png|json))$/.exec(pathname)
    const route = routes.get(pathname)??(derived?[`frontend/public${pathname}`,derived[2]==='png'?'image/png':'application/json']:undefined)
    if (!route || request.method !== 'GET') { response.writeHead(404); response.end(); return }
    try {
      const bytes = await readFile(`${root}${route[0]}`)
      const immutable=derived&&/-[a-f0-9]{16}\.png$/.test(pathname),etag=`"${createHash('sha256').update(bytes).digest('hex')}"`
      const headers={ 'Content-Type': route[1], 'Cache-Control': immutable?'public, max-age=31536000, immutable':derived?'no-cache':'no-store', 'X-Content-Type-Options': 'nosniff',ETag:etag }
      if(derived&&request.headers['if-none-match']===etag){response.writeHead(304,headers);response.end();return}
      response.writeHead(200, headers)
      response.end(bytes)
    } catch { response.writeHead(500); response.end('Review resource unavailable') }
  })
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const server = createArtReviewServer()
  server.listen(Number(process.env.MCR_ART_PORT ?? 4193), '127.0.0.1', () => console.log(`Review: http://127.0.0.1:${server.address().port}${base}review.html`))
}
