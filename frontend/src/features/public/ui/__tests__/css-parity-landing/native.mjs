// Actual production Landing/PublicFrame; existing explicit MemorySession fixture.
// No app component, background, character asset or canonical entry behavior replaced.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdir, readFile, writeFile, statfs, rm } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'

const root = resolve(import.meta.dirname, '../../../../../..')
const repository = resolve(root, '..')
const evidenceRoot = resolve(repository, 'docs/mcr/evidence/css-parity-landing')
const round = process.env.MCR_LANDING_ROUND ?? ''
assert(['', 'round-02'].includes(round))
const iteration = process.env.MCR_LANDING_ITERATION ?? ''
assert(['', 'final'].includes(iteration) && (!iteration || round))
const evidence = resolve(evidenceRoot, round, iteration)
const baseline = round ? 'a5c6cad52e630d61aa4ce581e7230c13cc5feb25' : '29a42ca1f5d86262ee18141d2bab75996af7e6fd'
const phase = process.env.MCR_LANDING_PHASE ?? 'candidate'
assert(['baseline', 'candidate'].includes(phase))
const cache = resolve(`/tmp/mcr-css-parity-landing${round ? '-round-02' : ''}`, phase)
const target = resolve(evidenceRoot, 'reference-original.jpg')
const digest = value => createHash('sha256').update(value).digest('hex')
const baselineCSS = execFileSync('git', ['show', `${baseline}:frontend/src/features/public/ui/public.css`], { cwd: repository, encoding: 'utf8' })
const worktreeCSS = await readFile(resolve(root, 'src/features/public/ui/public.css'))
const report = {
  phase, baseline, source: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim(),
  cssSHA256: digest(phase === 'baseline' ? baselineCSS : worktreeCSS),
  worktreeCSSSHA256: digest(worktreeCSS),
  targetSHA256: digest(await readFile(target)),
  result: 'FAIL', cases: [], errors: [],
  limits: 'Synthetic canonical readiness/session fixture, actual Landing/PublicFrame/routes. No live provider, backend, mobile-device, release or exact-art acceptance.',
}
if (round) {
  const protectedPaths = [
    'src/features/public/ui/Landing.tsx', 'src/features/public/ui/PublicFrame.tsx',
    'src/features/public/ui/landing-reference/PosterArtwork.tsx',
    'src/features/public/ui/landing-reference/reference-contours.json',
    'src/entities/avatar/selected-static/index.tsx',
    'src/entities/avatar/selected-static/selected-static.css',
    'src/entities/avatar/selected-static/release.ts',
    'src/shared/styles/base.css', 'src/shared/styles/cozy-identity-backgrounds-v1.css',
  ]
  report.protectedInputs = Object.fromEntries(await Promise.all(protectedPaths.map(async path =>
    [path, digest(await readFile(resolve(root, path)))])))
}
assert.equal(report.targetSHA256, '2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23')
const free = await statfs(repository)
assert(free.bavail * free.bsize >= 8 * 1024 ** 3, '8GiB continuous floor')
await mkdir(evidence, { recursive: true })
const server = await createServer({
  root, configLoader: 'runner', cacheDir: cache, logLevel: 'warn',
  plugins: phase === 'baseline' ? [{ name: 'owned-css-baseline', enforce: 'pre', transform(source, id) {
    if (id.split('?')[0] === resolve(root, 'src/features/public/ui/public.css')) return baselineCSS
  } }] : [],
  server: { host: '127.0.0.1', port: 5291, strictPort: true },
})
let browser
try {
  await server.listen()
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  report.browser = browser.version()
  const widths = phase === 'baseline' ? [390] : [360, 390, 430]
  for (const width of widths) {
    const free = await statfs(repository)
    assert(free.bavail * free.bsize >= 8 * 1024 ** 3)
    const height = width === 360 ? 800 : width === 390 ? 844 : 932
    const page = await browser.newPage({ viewport: { width, height }, reducedMotion: 'reduce' })
    page.on('pageerror', error => report.errors.push(error.message))
    const url = 'http://127.0.0.1:5291/src/features/public/__tests__/reference-target/preview.html'
    await page.goto(url)
    const entry = page.getByRole('link', { name: 'Sign in to enter', exact: true })
    await expect(entry).toHaveAttribute('href', '/login?returnTo=%2Fworld')
    await expect(page.locator('.landing-crowd')).toHaveAttribute('data-art-state', 'ready')
    await expect(page.locator('.landing-crowd > span')).toHaveCount(4)
    await page.evaluate(async () => {
      const image = new Image()
      image.src = getComputedStyle(document.querySelector('.landing-crowd > span')).backgroundImage.slice(5, -2)
      await image.decode()
      if (image.naturalWidth !== 384 || image.naturalHeight !== 64) throw new Error('Selected atlas changed')
      await document.fonts.ready
    })
    const geometry = await page.evaluate(() => {
      const rect = selector => {
        const r = document.querySelector(selector).getBoundingClientRect()
        return { x: r.x, y: r.y, width: r.width, height: r.height }
      }
      const poster = document.querySelector('.public-poster'), paint = getComputedStyle(poster)
      return {
        header: rect('.public-header'), brand: rect('.public-brand'), signIn: rect('.public-header > .public-text-link'),
        poster: rect('.public-poster'), wordmark: rect('.landing-wordmark'), copy: rect('.landing-poster-copy'), cta: rect('.public-primary'),
        characters: [...document.querySelectorAll('.landing-crowd > span')].map(el => {
          const s = getComputedStyle(el), r = el.getBoundingClientRect()
          return { x: r.x, y: r.y, width: r.width, height: r.height, backgroundPosition: s.backgroundPosition,
            backgroundSize: s.backgroundSize, imageRendering: s.imageRendering, transform: s.transform }
        }),
        background: { image: paint.backgroundImage, color: paint.backgroundColor, size: paint.backgroundSize },
        sceneOpacity: getComputedStyle(document.querySelector('.landing-scene > g')).opacity,
        overflow: document.documentElement.scrollWidth > innerWidth,
      }
    })
    assert(!geometry.overflow)
    assert(geometry.cta.height >= 44 && geometry.cta.width >= 44)
    assert(geometry.brand.height >= 44 && geometry.signIn.height >= 44)
    assert.equal(geometry.sceneOpacity, '0')
    assert(geometry.background.image.includes('landing-garden-night.svg'))
    assert.equal(await page.locator('canvas, [data-world-host]').count(), 0)
    const screenshots = []
    if (!iteration) {
      const file = `${phase}-landing-${width}.png`; screenshots.push(file)
      await page.screenshot({ path: resolve(evidence, file) })
    }
    if (!iteration || width === 390) {
      const file = `${phase}-poster-${width}.png`; screenshots.push(file)
      await page.screenshot({ path: resolve(evidence, file), clip: {
      x: geometry.header.x, y: geometry.header.y, width: geometry.header.width,
      height: geometry.poster.y + geometry.poster.height - geometry.header.y,
      } })
    }
    if (phase === 'candidate' && (width === 390 || round)) {
      const priorReport = JSON.parse(await readFile(resolve(width === 390 ? resolve(evidenceRoot, round) : evidenceRoot,
        width === 390 ? 'baseline.json' : 'candidate.json'), 'utf8'))
      const before = priorReport.cases.find(item => item.width === width).geometry
      assert.deepEqual(geometry.background, before.background)
      if (width === 390 && round) assert.deepEqual(report.protectedInputs, priorReport.protectedInputs)
      for (let i = 0; i < 4; i++) {
        const old = before.characters[i], current = geometry.characters[i]
        for (const key of ['width', 'height', 'backgroundPosition', 'backgroundSize', 'imageRendering', 'transform']) assert.equal(current[key], old[key], key)
        assert(Math.abs((current.y - geometry.poster.y) - (old.y - before.poster.y)) < .02)
        assert.equal(current.x, old.x)
      }
    }
    if (phase === 'candidate') {
      await page.keyboard.press('Tab')
      await expect(page.getByRole('link', { name: 'Skip to content' })).toBeFocused()
      await page.keyboard.press('Enter')
      await expect(page.locator('#public-main')).toBeFocused()
      for (let i = 0; i < 8 && !await entry.evaluate(el => el === document.activeElement); i++) await page.keyboard.press('Tab')
      await expect(entry).toBeFocused()
      assert.notEqual(await entry.evaluate(el => getComputedStyle(el).outlineStyle), 'none')
      if (width === 390) {
        const file = 'candidate-focused-390.png'
        await page.screenshot({ path: resolve(evidence, file) }); screenshots.push(file)
      }
      await page.getByText('About this space', { exact: true }).click()
      await expect(page.getByText('A place for the crowd.', { exact: true })).toBeVisible()
      await page.getByText('About this space', { exact: true }).click()
      await entry.click()
      await expect(page.getByRole('heading', { name: 'Sign in', exact: true })).toBeVisible()
      assert.equal(await page.evaluate(() => window.__REFERENCE_PUBLIC__.state().search), '?returnTo=%2Fworld')
      await page.evaluate(() => window.__REFERENCE_PUBLIC__.back())
      await expect(entry).toBeVisible()
      await page.getByRole('navigation', { name: 'Public information' }).getByRole('link', { name: 'Safety', exact: true }).click()
      await expect(page.getByRole('heading', { name: 'Safety', exact: true })).toBeFocused()
      assert.equal(await page.locator('.public-entry--poster').count(), 0)
      await page.getByRole('link', { name: 'Back to home', exact: true }).click()
      await expect(entry).toBeVisible()
      await page.setViewportSize({ width, height: 420 })
      await entry.scrollIntoViewIfNeeded(); await expect(entry).toBeInViewport()
      await page.setViewportSize({ width, height })
      await page.goto(`${url}?entry=unavailable`)
      await expect(page.getByRole('button', { name: 'Enter the world', exact: true })).toBeDisabled()
      await expect(page.getByText('World entry is not open yet.', { exact: true })).toBeVisible()
      if (width === 390 && !iteration) {
        const file = 'candidate-unavailable-390.png'
        await page.screenshot({ path: resolve(evidence, file) }); screenshots.push(file)
      }
      await page.goto(`${url}?entry=error`)
      await page.getByRole('button', { name: 'Check entry again', exact: true }).click()
      await expect(page.getByText('Checking your session…', { exact: true })).toBeVisible()
      assert.equal(await page.evaluate(() => window.__REFERENCE_PUBLIC__.state().retries), 1)
    }
    report.cases.push({ width, height, geometry, screenshots, interactions: phase === 'candidate' ? 'PASS' : 'NOT_RUN' })
    await page.close()
  }
  assert.deepEqual(report.errors, [])
  report.result = 'PASS'
} catch (error) {
  report.failure = String(error.stack ?? error)
  process.exitCode = 1
} finally {
  await browser?.close()
  await server.close()
  await writeFile(resolve(evidence, `${phase}.json`), JSON.stringify(report, null, 2) + '\n')
  await rm(cache, { recursive: true, force: true })
}
console.log(JSON.stringify({ phase, result: report.result, cases: report.cases.length, failure: report.failure }))
