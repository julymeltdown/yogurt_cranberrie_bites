// Baseline only: build the unchanged real entry and explicitly labeled fixtures.
import { build, preview } from 'vite'
import { chromium } from '@playwright/test'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import { mkdtemp, rm, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
const dir = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(dir, '../../../../../..'), frontend = path.join(root, 'frontend')
const outDir = await mkdtemp(path.join(tmpdir(), 'mcr-uiv02-before-'))
const base = '453312121ae7539c027b9deb19009f9f569d4040'
const sourcePaths = ['frontend/src/features/public/ui/Landing.tsx', 'frontend/src/features/public/ui/PublicFrame.tsx',
  'frontend/src/features/public/ui/public.css', 'frontend/src/features/auth/ui/SignIn.tsx', 'frontend/src/features/auth/ui/sign-in.css']
let server, browser
try {
  // Never overwrite historical baseline captures using the adopted presentation.
  execFileSync('git', ['diff', '--exit-code', base, '--', ...sourcePaths], { cwd: root, stdio: 'pipe' })
  const sourceHashes = {}
  for (const p of sourcePaths) sourceHashes[p] = createHash('sha256').update(await readFile(path.join(root, p))).digest('hex')
  await build({ configFile: false, root: frontend, plugins: [react(), tailwindcss()],
    build: { outDir, rollupOptions: { input: [path.join(frontend, 'index.html'), path.join(frontend, 'src/features/auth/__tests__/visual-adoption/preview.html')] } } })
  server = await preview({ configFile: false, root: frontend, build: { outDir }, preview: { host: '127.0.0.1', port: 0 } })
  const origin = `http://127.0.0.1:${server.httpServer.address().port}`
  const caps = JSON.parse(await readFile(path.join(root, 'contracts/fixtures/core/auth-capabilities-disabled.json')))
  caps.providers[0] = { provider: 'EMAIL', enabled: true, disabled_reason: null }
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const records = []
  for (const width of [360, 390, 430]) {
    const context = await browser.newContext({ viewport: { width, height: 844 } }), page = await context.newPage()
    for (const [label, url] of [['home', '/'], ['login', '/login'], ['populated', '/src/features/auth/__tests__/visual-adoption/preview.html']]) {
      await page.route('**/api/auth/capabilities', route => route.fulfill({ status: 200, json: caps }))
      await page.goto(origin + url)
      await page.getByRole('heading', { level: 1 }).waitFor()
      if (label === 'populated') await page.getByRole('button', { name: 'Sign in with email' }).waitFor()
      const filename = `before-${label}-${width}.png`
      const folder = path.join(frontend, `src/features/${label === 'home' ? 'public' : 'auth'}/__tests__/visual-adoption/evidence`)
      await page.screenshot({ path: path.join(folder, filename), fullPage: true })
      records.push({ filename, width, sha256: createHash('sha256').update(await readFile(path.join(folder, filename))).digest('hex') })
    }
    await context.close()
  }
  await writeFile(path.join(dir, 'evidence/before.json'), JSON.stringify({ base, sourceHashes, browser: browser.version(), records }, null, 2) + '\n')
} finally {
  await browser?.close()
  await new Promise(resolve => server ? server.httpServer.close(resolve) : resolve())
  await rm(outDir, { recursive: true, force: true })
}
