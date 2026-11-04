import assert from 'node:assert/strict'
import { writeFile, mkdir } from 'node:fs/promises'
import { isAbsolute, join } from 'node:path'
import { chromium } from '@playwright/test'

const origin = process.env.MCR_FEED_ORIGIN, output = process.env.MCR_FEED_OUTPUT
if (!origin || !output || !isAbsolute(output) || !process.env.MCR_FEED_ALLOCATION) throw Error('Use the accepted Feed native allocation')
const url = new URL(origin)
assert.equal(url.hostname, '127.0.0.1'); assert.equal(url.pathname, '/')
await mkdir(output, { recursive: true })
const browser = await chromium.launch({ channel: 'chrome', headless: true })
const results = [], errors = []
const remaining = process.env.MCR_FEED_REMAINING === '1'
async function open(mode, width, height) {
  const context = await browser.newContext({ viewport: { width, height }, deviceScaleFactor: 1, reducedMotion: 'reduce' })
  const page = await context.newPage()
  page.on('pageerror', error => errors.push(error.message))
  await page.clock.setFixedTime(new Date('2026-11-04T12:00:00Z'))
  await page.goto(`${origin}src/app/feed-entry/__tests__/preview.html?mode=${mode}`, { waitUntil: 'domcontentloaded' })
  await page.waitForFunction(() => Boolean(window.__FEED_SHELL__))
  await page.locator('.shell-panel').waitFor()
  return { context, page }
}
async function metrics(page) {
  return page.evaluate(() => {
    const rect = element => {
      const r = element.getBoundingClientRect(), css = getComputedStyle(element)
      return { x: r.x, y: r.y, width: r.width, height: r.height, scrollHeight: element.scrollHeight, clientHeight: element.clientHeight,
        background: css.backgroundColor, backgroundImage: css.backgroundImage, overflowY: css.overflowY, objectFit: css.objectFit,
        aspectRatio: css.aspectRatio, minBlockSize: css.minBlockSize }
    }
    return { viewport: { width: innerWidth, height: innerHeight },
      regions: Object.fromEntries(['.world-shell', '.shell-panel', '.mcr-chrome-panel-header', '.shell-panel-body', '.event-feed', '.feed-footer'].map(selector => [selector, document.querySelector(selector) ? rect(document.querySelector(selector)) : null])),
      images: [...document.querySelectorAll('.feed-image')].map(image => ({ ...rect(image), naturalWidth: image.naturalWidth, naturalHeight: image.naturalHeight, blob: image.src.startsWith('blob:') })),
      mediaFrames: [...document.querySelectorAll('.feed-media .root-media-view--post')].map(rect),
      authors: [...document.querySelectorAll('.feed-portrait')].map(element => {
        const canvas = element.querySelector('canvas'), pixels = canvas?.getContext('2d')?.getImageData(0, 0, canvas.width, canvas.height).data
        return { ...rect(element), canvas: canvas ? rect(canvas) : null, ready: element.querySelector('[data-whole-character-state]')?.dataset.wholeCharacterState,
          paintedPixels: pixels ? pixels.filter((alpha, index) => index % 4 === 3 && alpha > 0).length : 0 }
      }),
      horizontalOverflow: document.documentElement.scrollWidth > innerWidth,
      calls: window.__FEED_SHELL__.calls, blobUrls: window.__FEED_SHELL__.resources(), characters: window.__FEED_SHELL__.characterDiagnostics(), qualification: window.__FEED_SHELL__.qualification }
  })
}
async function scrollProbe(page, name) {
  const first = page.locator('.feed-posts > li').first().locator('.root-media-view--post')
  await first.scrollIntoViewIfNeeded()
  await first.locator('.feed-image').waitFor()
  await page.evaluate(() => {
    const spacer = document.createElement('div')
    spacer.id = 'feed-driver-scroll-probe'; spacer.setAttribute('aria-hidden', 'true')
    spacer.style.height = `${innerHeight * 2}px`
    document.querySelector('.event-feed').append(spacer)
  })
  const sample = () => page.evaluate(() => {
    const body = document.querySelector('.shell-panel-body'), frame = document.querySelector('.feed-media .root-media-view--post')
    return { frameHeight: frame.getBoundingClientRect().height, frameTop: frame.getBoundingClientRect().top,
      scrollHeight: body.scrollHeight, scrollTop: body.scrollTop, imagePresent: Boolean(frame.querySelector('.feed-image')) }
  })
  try {
    const before = await sample()
    await first.evaluate(frame => {
      const body = document.querySelector('.shell-panel-body')
      body.scrollTop += frame.getBoundingClientRect().bottom - body.getBoundingClientRect().top + 350
    })
    await page.waitForFunction(() => !document.querySelector('.feed-media .root-media-view--post .feed-image'))
    const outside = await sample()
    if (name.endsWith('390x480')) await page.screenshot({ path: join(output, `${name}-scroll-out.png`) })
    await first.scrollIntoViewIfNeeded()
    await page.waitForFunction(() => document.querySelector('.feed-media .root-media-view--post .feed-image')?.naturalWidth === 197)
    const returned = await sample()
    return { driverOnlySpacer: true, observerMargin: 200, before, outside, returned,
      geometryPreservedOnExit: Math.abs(before.frameHeight - outside.frameHeight) <= 1,
      geometryRestoredOnReturn: Math.abs(before.frameHeight - returned.frameHeight) <= 1 }
  } finally {
    await page.evaluate(() => { document.querySelector('#feed-driver-scroll-probe')?.remove(); document.querySelector('.shell-panel-body').scrollTop = 0 })
  }
}
try {
  for (const [width, height] of remaining ? [[390, 480]] : [[360, 843], [390, 843], [430, 843], [390, 480]]) {
    const { context, page } = await open('loaded', width, height)
    try {
      await page.locator('.feed-posts > li').last().scrollIntoViewIfNeeded()
      await page.waitForFunction(() => document.querySelectorAll('.feed-image').length === 2 && [...document.querySelectorAll('.feed-image')].every(image => image.complete && image.naturalWidth === 197))
      await page.waitForFunction(() => document.querySelectorAll('.feed-portrait [data-whole-character-state="ready"]').length === 2)
      await page.locator('.shell-panel-body').evaluate(element => { element.scrollTop = 0 })
      const name = `feed-${width}x${height}`
      const observed = await metrics(page)
      assert.equal(observed.horizontalOverflow, false)
      assert.equal(observed.regions['.shell-panel-body'].background, 'rgb(248, 245, 236)')
      assert.equal(observed.regions['.shell-panel-body'].backgroundImage, 'none')
      assert.deepEqual(observed.images.map(image => [image.naturalWidth, image.naturalHeight]), [[197, 67], [197, 46]])
      assert(observed.images.every(image => image.blob))
      if (process.env.MCR_FEED_COMPACT_MEDIA === '1') {
        assert.equal(observed.mediaFrames.length, 2)
        observed.mediaFrames.forEach((frame, index) => {
          assert.equal(frame.aspectRatio, 'auto'); assert.equal(frame.minBlockSize, '0px')
          assert(frame.height <= observed.images[index].height + 12, 'Feed media wrapper introduced blank letterbox space')
        })
      }
      assert(observed.authors.every(author => author.ready === 'ready' && author.paintedPixels > 0))
      assert.equal(observed.characters?.decodedAtlases, 1)
      await page.screenshot({ path: join(output, `${name}.png`), fullPage: false })
      for (let index = 0; index < 2; index++) {
        const row = page.locator('.feed-posts > li').nth(index)
        await row.scrollIntoViewIfNeeded()
        await row.locator('.feed-image').screenshot({ path: join(output, `${name}-media-${index + 1}.png`), timeout: 5000 })
      }
      for (let index = 0; index < 2; index++) await page.locator('.feed-portrait').nth(index).screenshot({ path: join(output, `${name}-author-${index + 1}.png`) })
      const scrolling = await scrollProbe(page, name)
      const compose = page.getByRole('button', { name: 'Write a post', exact: true })
      await compose.scrollIntoViewIfNeeded(); await compose.click()
      await page.getByRole('textbox').first().waitFor()
      const composerReachable = true
      await page.evaluate(() => window.__FEED_SHELL__.navigate('/feed'))
      await page.locator('.feed-posts > li').first().waitFor()
      await page.evaluate(() => window.__FEED_SHELL__.setReady(false))
      await page.waitForFunction(() => document.querySelectorAll('.feed-posts > li').length === 0)
      await page.evaluate(() => window.__FEED_SHELL__.dispose())
      assert.equal(await page.evaluate(() => window.__FEED_SHELL__.resources()), 0)
      results.push({ name, ...observed, scrolling, composerReachable, readinessWithdrawal: true, disposedBlobUrls: 0 })
    } catch (error) {
      await page.screenshot({ path: join(output, `failed-${width}x${height}.png`) }).catch(() => {})
      throw error
    } finally { await context.close() }
  }
  for (const mode of ['no-readiness', 'denied-media']) {
    const { context, page } = await open(mode, 390, 843)
    try {
      if (mode === 'no-readiness') {
        await page.getByText('This section is unavailable for the current account or event.', { exact: true }).waitFor()
        assert.equal(await page.locator('.feed-posts > li').count(), 0)
        assert.equal(await page.evaluate(() => window.__FEED_SHELL__.calls.filter(call => call.path.endsWith('/feed')).length), 0)
      } else {
        await page.locator('.feed-posts > li').last().scrollIntoViewIfNeeded()
        await page.waitForFunction(() => window.__FEED_SHELL__.calls.filter(call => call.path.includes('/media/') && call.status === 403).length === 2)
        assert.equal(await page.locator('.feed-image').count(), 0)
        assert.equal(await page.evaluate(() => window.__FEED_SHELL__.calls.filter(call => /\/media\/[^/]+\/content$/.test(call.path)).length), 0)
      }
      await page.screenshot({ path: join(output, `${mode}-390x843.png`) })
      const observed = await metrics(page)
      await page.evaluate(() => window.__FEED_SHELL__.invalidate())
      await page.waitForFunction(() => window.__FEED_SHELL__.resources() === 0 && document.querySelectorAll('.feed-image').length === 0)
      await page.evaluate(() => window.__FEED_SHELL__.dispose())
      results.push({ mode, ...observed, invalidationWithdrawal: true })
    } finally { await context.close() }
  }
  assert.deepEqual(errors, [])
  assert.deepEqual(results.filter(result => result.scrolling && (!result.scrolling.geometryPreservedOnExit || !result.scrolling.geometryRestoredOnReturn)).map(result => result.name), [], 'Offscreen media withdrawal changed Feed geometry')
} finally {
  await browser.close()
  await writeFile(join(output, 'native.json'), JSON.stringify({ synthetic: true, remainingCasesOnly: remaining, browser: browser.version(), liveIN03: 'NOT_RUN', results, errors }, null, 2) + '\n')
}
