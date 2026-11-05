import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { isAbsolute, join } from 'node:path'
import { chromium } from '@playwright/test'
const origin = process.env.MCR_PI_ORIGIN, output = process.env.MCR_PI_OUTPUT
if (!origin || !output || !isAbsolute(output) || !process.env.MCR_PI_ALLOCATION) throw Error('Accepted PI02/03 native allocation required')
assert.equal(new URL(origin).hostname, '127.0.0.1')
await mkdir(output, { recursive: true })
const browser = await chromium.launch({ channel: 'chrome', headless: true })
const report = { status: 'IN_PROGRESS', startedAt: new Date().toISOString(), browser: browser.version(),
  qualification: 'ACTUAL_DEFAULT_APP_WITH_PINNED_PI01_OVERLAY_CONTROLLED_SYNTHETIC_HTTP_NOT_LIVE_IN03', cases: [], screenshots: [] }
const retainedPath = process.env.MCR_PI_RETAINED_NATIVE_REPORT
const retained = retainedPath ? JSON.parse(await readFile(retainedPath, 'utf8')) : null
if (retained) {
  assert.equal(retained.qualification, report.qualification)
  assert.equal(retained.status, 'FAIL')
  for (const name of ['real-default-navigation-edit-comment-edit-post-publish', 'long', 'empty', 'error', 'denied-media', 'read-only', 'notice', 'pending']) {
    assert(retained.cases.some(value => value.name === name && value.status === 'PASS'))
  }
  report.retained = { path: retainedPath, candidate: retained.screenshots[0].metrics.qualification.candidate,
    states: retained.cases.filter(value => value.status === 'PASS'), limitation: 'Prior overall FAIL is retained; only individually completed cases are reused.' }
}
const footerOnly = process.env.MCR_PI_FOOTER_ONLY === '1'
const errors = []
async function metrics(page) {
  return page.evaluate(() => {
    const rect = element => { const r = element.getBoundingClientRect(), css = getComputedStyle(element)
      return { x: r.x, y: r.y, width: r.width, height: r.height, color: css.color, background: css.backgroundColor,
        font: css.fontFamily, size: css.fontSize, lineHeight: css.lineHeight, border: css.borderTopWidth,
        scrollHeight: element.scrollHeight, clientHeight: element.clientHeight } }
    const selectors = ['.shell-panel', '.mcr-chrome-panel-header', '.shell-panel-body', '.event-feed', '.mcr-tablist',
      '.mcr-tab[aria-selected="true"]', '.feed-post', '.feed-author-line', '.feed-post-copy', '.feed-media', '.feed-footer', '.feed-compose', '.feed-reply']
    return { viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio },
      regions: Object.fromEntries(selectors.map(selector => [selector, document.querySelector(selector) ? rect(document.querySelector(selector)) : null])),
      controls: [...document.querySelectorAll('.event-feed button, .event-feed summary')].filter(element => element.getClientRects().length)
        .map(element => ({ label: element.getAttribute('aria-label') ?? element.textContent?.trim(), ...rect(element) })),
      portraits: [...document.querySelectorAll('.feed-portrait')].map(element => {
        const canvas = element.querySelector('canvas'), pixels = canvas?.getContext('2d')?.getImageData(0, 0, canvas.width, canvas.height).data
        return { ...rect(element), paintedPixels: pixels ? pixels.filter((alpha, i) => i % 4 === 3 && alpha > 0).length : 0,
          state: element.querySelector('[data-whole-character-state]')?.getAttribute('data-whole-character-state') }
      }),
      media: [...document.querySelectorAll('.feed-media img')].map(element => ({ ...rect(element), naturalWidth: element.naturalWidth,
        naturalHeight: element.naturalHeight, blob: element.src.startsWith('blob:') })),
      safeAreaPadding: { top: getComputedStyle(document.querySelector('.world-shell')).paddingTop, bottom: getComputedStyle(document.querySelector('.world-shell')).paddingBottom },
      horizontalOverflow: document.documentElement.scrollWidth > innerWidth,
      calls: [...window.__PARADE_FEED__.calls], urls: window.__PARADE_FEED__.resources(),
      characters: window.__PARADE_FEED__.characterDiagnostics(), qualification: window.__PARADE_FEED__.qualification,
      piContract: getComputedStyle(document.documentElement).getPropertyValue('--pi-contract').trim(),
      fonts: [...document.fonts].map(face => ({ family: face.family, status: face.status })),
      fontReady: ['Parade Ink Text', 'Parade Ink Display'].every(name => [...document.fonts].some(face => face.family.replaceAll('"', '') === name && face.status === 'loaded')) }
  })
}
async function open(mode, width = 390, height = 844, safeArea = false) {
  const context = await browser.newContext({ viewport: { width, height }, deviceScaleFactor: 1, reducedMotion: 'reduce' })
  const page = await context.newPage()
  if (safeArea) {
    const cdp = await context.newCDPSession(page)
    await cdp.send('Emulation.setSafeAreaInsetsOverride', { insets: { top: 24, bottom: 34, left: 0, right: 0 } })
  }
  page.on('pageerror', error => errors.push(error.message))
  page.setDefaultTimeout(10000)
  await page.clock.setFixedTime(new Date('2026-11-04T12:00:00Z'))
  await page.goto(`${origin}src/features/event-feed/__tests__/parade-ink/preview.html?mode=${mode}`, { waitUntil: 'domcontentloaded' })
  await page.waitForFunction(() => Boolean(window.__PARADE_FEED__))
  await page.locator('.shell-panel').waitFor()
  await page.evaluate(() => document.fonts.ready)
  return { context, page }
}
async function capture(page, name) {
  try { await page.waitForFunction(() => [...document.querySelectorAll('.feed-media figure')].every(frame => {
    if (window.__PARADE_FEED__.qualification.mode === 'denied-media') return true
    const visibility = frame.querySelector('[data-root-media-visibility]')?.getAttribute('data-root-media-visibility')
    if (visibility === 'outside') return true // The incumbent provider intentionally unloads offscreen private media.
    const image = frame.querySelector('img'); return image?.complete && image.naturalWidth > 0
  })) } catch (error) {
    report.captureWaitFailure = { name, metrics: await metrics(page), frames: await page.locator('.feed-media').evaluateAll(frames => frames.map(frame => ({ text: frame.textContent, html: frame.innerHTML }))) }
    throw error
  }
  const record = { file: `${name}.png`, capturedAt: new Date().toISOString(), metrics: await metrics(page) }
  assert(!record.metrics.horizontalOverflow, `${name}: horizontal overflow`)
  assert(record.metrics.fontReady, `${name}: bundled fonts unavailable`)
  for (const control of record.metrics.controls) assert(control.height >= 43.5, `${name}: undersized ${control.label}`)
  await page.screenshot({ path: join(output, record.file) })
  report.screenshots.push(record)
  await writeFile(join(output, 'report.partial.json'), JSON.stringify(report, null, 2))
  return record
}
async function end(context, page, name) {
  await page.evaluate(() => window.__PARADE_FEED__.dispose())
  assert.equal(await page.evaluate(() => window.__PARADE_FEED__.resources()), 0, `${name}: private Blob leak`)
  await context.close()
}
try {
  for (const width of [360, 390, 430]) {
    const { context, page } = await open('loaded', width)
    await page.locator('.feed-post').first().waitFor()
    await page.locator('.feed-media img').first().waitFor()
    await page.waitForFunction(() => [...document.querySelectorAll('.feed-portrait canvas')].length >= 3)
    const feed = await capture(page, `feed-${width}`)
    assert(feed.metrics.portraits.every(portrait => portrait.paintedPixels > 0), 'Approved portraits must actually paint')
    assert.equal(feed.metrics.regions['.mcr-tab[aria-selected="true"]'].background, 'rgb(23, 23, 23)')
    const toolbar = feed.metrics.regions['.feed-footer']
    assert(toolbar.y >= 0 && toolbar.y + toolbar.height <= feed.metrics.viewport.height, 'Feed toolbar must stay visible without scrolling')
    const panelBody = feed.metrics.regions['.shell-panel-body']
    assert(Math.abs(toolbar.y + toolbar.height - (panelBody.y + panelBody.height - 6)) < 1, 'Feed must fill the allocated panel down to its bottom inset')
    if (footerOnly) {
      const compose = feed.metrics.regions['.feed-compose']
      assert(Math.abs(compose.width - width * 231 / 395) < 2, 'New Post width must match the normalized primary reference')
      assert(Math.abs(compose.x - width * 87 / 395) < 2, 'New Post position must match the normalized primary reference')
      report.cases.push({ name: `footer-primary-reference-${width}`, status: 'PASS' })
      await end(context, page, `footer-${width}`)
      continue
    }
    if (width === 390 && !retained) {
      await page.locator('.feed-tools summary').click()
      await page.getByRole('checkbox', { name: 'Images only' }).check()
      await page.waitForFunction(() => document.querySelectorAll('.feed-post').length === 2)
      await page.getByRole('checkbox', { name: 'Images only' }).uncheck()
      await page.waitForFunction(() => document.querySelectorAll('.feed-post').length === 3)
      await page.locator('.feed-tools summary').click()
      await page.getByRole('button', { name: 'Refresh feed', exact: true }).click()
      await page.getByRole('button', { name: 'Open thread · 7 comments' }).waitFor()
    }
    await page.getByRole('button', { name: 'Open thread · 7 comments' }).click()
    await page.getByRole('heading', { name: 'Comments', exact: true }).waitFor()
    await page.getByText('A test reply', { exact: true }).waitFor()
    await capture(page, `thread-${width}`)
    if (width === 390 && !retained) {
      await page.getByRole('button', { name: 'Edit comment', exact: true }).click()
      await page.getByRole('textbox', { name: 'Edit comment text' }).fill('An edited controlled reply')
      await page.getByRole('button', { name: 'Save comment changes' }).click()
      await page.getByText('An edited controlled reply', { exact: true }).waitFor()
      await page.getByRole('button', { name: 'Delete comment', exact: true }).click()
      const before = await page.evaluate(() => window.__PARADE_FEED__.calls.filter(call => call.method === 'DELETE').length)
      await page.getByRole('button', { name: 'Keep content' }).click()
      assert.equal(await page.evaluate(() => window.__PARADE_FEED__.calls.filter(call => call.method === 'DELETE').length), before)
      await page.getByRole('button', { name: 'Edit post', exact: true }).click()
      await page.getByRole('textbox', { name: 'Post text' }).waitFor()
      await capture(page, 'composer-edit-390')
      await page.getByRole('textbox', { name: 'Post text' }).fill('A controlled native post edit')
      await page.getByRole('button', { name: 'Save post changes' }).click()
      await page.getByRole('button', { name: 'Back to post', exact: true }).click()
      await page.getByText('A controlled native post edit', { exact: true }).waitFor()
      await page.evaluate(() => window.__PARADE_FEED__.navigate('/feed'))
      await page.getByRole('button', { name: 'Write a post' }).click()
      await page.getByRole('textbox', { name: 'Post text' }).fill('A controlled newly published post')
      await page.getByRole('button', { name: 'Publish post', exact: true }).click()
      await page.getByRole('button', { name: 'Open published post' }).click()
      await page.getByText('A controlled newly published post', { exact: true }).waitFor()
      await page.getByRole('button', { name: 'Delete post', exact: true }).click()
      await page.getByRole('group', { name: 'Confirm deletion', exact: true }).waitFor()
      await page.getByRole('button', { name: 'Keep content' }).click()
      await page.getByText('A controlled newly published post', { exact: true }).waitFor()
      await page.getByRole('button', { name: 'Delete post', exact: true }).click()
      await page.getByRole('button', { name: 'Confirm deletion', exact: true }).click()
      await page.getByText('The server confirmed this change.', { exact: true }).waitFor()
      assert.equal(await page.getByText('A controlled newly published post', { exact: true }).count(), 0)
      report.cases.push({ name: 'real-default-navigation-edit-comment-edit-post-publish', status: 'PASS' })
    }
    await end(context, page, `loaded-${width}`)
  }
  for (const mode of (footerOnly ? [] : retained ? ['no-readiness'] : ['long', 'empty', 'error', 'denied-media', 'read-only', 'notice', 'pending', 'no-readiness'])) {
    const { context, page } = await open(mode)
    if (mode === 'empty') await page.getByText('No posts yet.', { exact: true }).waitFor()
    else if (mode === 'error') await page.getByRole('button', { name: 'Retry read' }).first().waitFor()
    else if (mode === 'notice') {
      await page.getByRole('tab', { name: 'NOTICE', exact: true }).click()
      await page.getByRole('button', { name: 'Fixture notice', exact: true }).click()
      await page.locator('.feed-notice-detail').waitFor()
    } else if (mode === 'pending') {
      await page.getByRole('button', { name: 'Open thread · 7 comments' }).click()
      await page.getByRole('textbox', { name: 'Comment text' }).fill('A pending controlled reply')
      await page.getByRole('button', { name: 'Send comment' }).click()
      await page.getByRole('button', { name: 'Check command status' }).waitFor()
      assert(await page.getByRole('button', { name: 'Send comment' }).isDisabled())
    } else if (mode === 'no-readiness') {
      await page.getByText('This section is unavailable for the current account or event.', { exact: true }).waitFor()
      assert.equal(await page.locator('.feed-post').count(), 0)
      assert.equal(await page.evaluate(() => window.__PARADE_FEED__.calls.filter(call => /\/(feed|posts|media)(\/|$)/.test(call.path)).length), 0)
    }
    else await page.locator('.feed-post').first().waitFor()
    if (mode === 'denied-media') await page.waitForFunction(() => window.__PARADE_FEED__.calls.some(call => call.path.includes('/media/') && call.status === 403))
    if (mode !== 'no-readiness') await capture(page, `${mode}-390`)
    if (mode === 'read-only') assert.equal(await page.getByRole('button', { name: 'Write a post' }).count(), 0)
    if (mode === 'pending') {
      await page.getByRole('button', { name: 'Check command status' }).click()
      await page.getByText('The server confirmed this change.', { exact: true }).waitFor()
      await page.getByText('A pending controlled reply', { exact: true }).waitFor()
    }
    await end(context, page, mode)
    report.cases.push({ name: mode, status: 'PASS' })
  }
  {
    const { context, page } = await open('loaded', 390, 480, true)
    await page.locator('.feed-post').first().waitFor()
    const compose = page.getByRole('button', { name: 'Write a post' })
    await compose.focus(); await page.keyboard.press('Tab'); await page.keyboard.press('Shift+Tab')
    assert(await compose.evaluate(element => element === document.activeElement && element.matches(':focus-visible')))
    await compose.scrollIntoViewIfNeeded()
    const short = await capture(page, 'short-focus-390')
    assert.deepEqual(short.metrics.safeAreaPadding, { top: '26px', bottom: '36px' }, 'Browser safe-area overrides plus the 2px frame keyline must reach the actual shell')
    if (!footerOnly) {
    await compose.click()
    await page.getByRole('textbox', { name: 'Post text' }).fill('Short viewport keyboard draft')
    await page.getByRole('button', { name: 'Publish post' }).scrollIntoViewIfNeeded()
    await capture(page, 'short-composer-390')
    }
    await end(context, page, 'short-focus')
    report.cases.push({ name: footerOnly ? 'changed-footer-keyboard-focus-safe-area-24-34' : 'short-height-keyboard-focus-safe-area-24-34-and-composer-access', status: 'PASS' })
  }
  assert.deepEqual(errors, [])
  report.status = 'PASS'
} catch (error) {
  report.status = 'FAIL'; report.error = error.stack ?? String(error)
  throw error
} finally {
  report.finishedAt = new Date().toISOString(); report.pageErrors = errors
  await writeFile(join(output, 'report.json'), JSON.stringify(report, null, 2))
  await browser.close()
}
