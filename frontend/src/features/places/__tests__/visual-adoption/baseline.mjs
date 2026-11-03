import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { chromium, expect } from '@playwright/test'
import { resolve } from 'node:path'
import { writeFile, readFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { bundle, serve, fixturePath } from './build.mjs'
const evidence = resolve(import.meta.dirname, 'evidence')
const root = resolve(import.meta.dirname, '../../../../../..')
for (const file of ['frontend/src/features/places/ui/Places.tsx','frontend/src/features/places/ui/places.css']) {
  const expected = execFileSync('git',['show',`bea2e72f4d6f0e42be32700b5546c266bb157231:${file}`],{cwd:root})
  assert.equal(createHash('sha256').update(await readFile(resolve(root,file))).digest('hex'),createHash('sha256').update(expected).digest('hex'),'Before capture requires exact assignment-base UI bytes')
}
await bundle(); const server = await serve(); let browser
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  const report = { browser: browser.version(), scope: 'UIV05 before: exact base Places presentation; FE19-derived synthetic native shell fixture', sourceHashes: {}, screenshots: [] }
  for (const file of ['../../ui/Places.tsx','../../ui/places.css','../../../../../../docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg']) {
    report.sourceHashes[file] = createHash('sha256').update(await readFile(resolve(import.meta.dirname, file))).digest('hex')
  }
  for (const [width,height] of [[360,800],[390,844],[430,932]]) {
    const context = await browser.newContext({ viewport: { width,height }, hasTouch: true, reducedMotion: 'reduce' }), page = await context.newPage()
    await page.goto(`http://127.0.0.1:${server.httpServer.address().port}${fixturePath}`)
    await expect(page.getByRole('link', { name: 'View Synthetic virtual stage' })).toBeVisible()
    await page.getByRole('button', { name: 'Expand panel' }).click()
    await expect(page.getByRole('img', { name: /Virtual map overview/ })).toBeVisible()
    const file = `before-overview-${width}.png`; await page.screenshot({ path: resolve(evidence,file), fullPage: true }); report.screenshots.push(file)
    await page.evaluate(() => window.__UIV05_MAP__.dispose()); await context.close()
  }
  await writeFile(resolve(evidence,'baseline.json'), JSON.stringify(report,null,2)+'\n')
} finally { await browser?.close(); await new Promise(done=>server.httpServer.close(done)) }
