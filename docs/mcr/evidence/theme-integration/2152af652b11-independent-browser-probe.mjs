import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';

const root = '/Users/lhs/Desktop/mcr-worktrees/qa-fe-theme/frontend';
const require = createRequire(root + '/package.json');
const { createServer } = await import(require.resolve('vite'));
const { chromium, expect } = require('@playwright/test');

let resourceMode = 'valid';
const errors = [];
const requests = [];

const dev = await createServer({
  root,
  server: { host: '127.0.0.1', port: 45849, strictPort: true },
  plugins: [{
    name: 'qa-synthetic-resources',
    configureServer(server) {
      server.middlewares.use(async (req, res, next) => {
        const match = /^\/world\/fe06\/(revenge|parade|danger|map)\.(png|json)$/.exec(req.url ?? '');
        if (!match) return next();
        requests.push({ url: req.url, cookie: req.headers.cookie ?? null });
        const { syntheticRelease } = await server.ssrLoadModule('/src/world/theme/__tests__/fixtures/releases.ts');
        const f = await syntheticRelease(match[1] === 'map' ? 'REVENGE' : match[1].toUpperCase());
        const png = match[2] === 'png';
        const bad = resourceMode === 'failure' && match[1] === 'parade';
        res.setHeader('Content-Type', png ? 'image/png' : 'application/json');
        res.setHeader('Cache-Control', 'no-store');
        res.statusCode = bad ? 503 : 200;
        res.end(png ? f.png : f.tiledBytes);
      });
    }
  }]
});

function track() {
  const urls = new Set();
  const origCreate = URL.createObjectURL;
  const origRevoke = URL.revokeObjectURL;
  URL.createObjectURL = blob => { const u = origCreate.call(URL, blob); urls.add(u); return u; };
  URL.revokeObjectURL = u => { urls.delete(u); origRevoke.call(URL, u); };
  const raf = requestAnimationFrame, caf = cancelAnimationFrame, frames = new Set();
  window.requestAnimationFrame = cb => { let id; id = raf(t => { frames.delete(id); cb(t); }); frames.add(id); return id; };
  window.cancelAnimationFrame = id => { frames.delete(id); caf(id); };
  window.__FE06_RESOURCES__ = () => ({ urls: urls.size, frames: frames.size });
}

let browser;
try {
  await dev.listen();
  const url = 'http://127.0.0.1:45849/src/features/theme/__tests__/preview.html';
  browser = await chromium.launch({
    executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    headless: true
  });

  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, reducedMotion: 'reduce' });
  const page = await context.newPage();
  await page.addInitScript(track);
  page.on('pageerror', err => errors.push(err.message));

  await page.goto(url);
  await expect(page.getByLabel('Runtime status')).toHaveText('ready');
  await expect(page.locator('canvas')).toHaveCount(1);

  await page.locator('canvas').evaluate(c => { window.__FE06_CANVAS__ = c; });

  const state = () => page.evaluate(() => window.__FE06__.state());
  const before = await state();
  assert.equal(before.skin, 'REVENGE');
  assert.equal(before.allocations, 1);
  assert.equal(before.attachments, 1);

  // Focus radio and arrow right to PARADE
  await page.getByRole('radio', { name: 'REVENGE', exact: true }).focus();
  await page.keyboard.press('ArrowRight');
  await expect.poll(async () => (await state()).skin).toBe('PARADE');
  await expect.poll(async () => (await state()).theme.selected).toBe('PARADE');

  // Verify canvas was NOT recreated
  assert(await page.locator('canvas').evaluate(c => c === window.__FE06_CANVAS__), 'Canvas recreated');
  assert.equal((await state()).allocations, 1);
  assert.equal((await state()).attachments, 1);

  // Arrow right to DANGER
  await page.keyboard.press('ArrowRight');
  await expect.poll(async () => (await state()).skin).toBe('DANGER');
  await expect.poll(async () => (await state()).theme.selected).toBe('DANGER');
  assert(await page.locator('canvas').evaluate(c => c === window.__FE06_CANVAS__), 'Canvas recreated');

  // Verify labels >= 44px
  for (const label of await page.locator('.mcr-theme-selector label').all()) {
    const box = await label.boundingBox();
    assert(box.width >= 44 && box.height >= 44);
  }

  // Rollback on simulated 503
  resourceMode = 'failure';
  await page.getByRole('radio', { name: 'PARADE', exact: true }).click();
  await expect(page.getByRole('status').first()).toContainText('DANGER remains selected');
  assert.equal((await state()).skin, 'DANGER');
  assert.equal(await page.evaluate(() => localStorage.getItem('mcr:theme:v1:synthetic-user:synthetic-event')), 'DANGER');
  resourceMode = 'valid';

  // Take screenshot
  await page.screenshot({ path: '/tmp/mcr-qa-fe06-agy8u3i5/qa-theme-390.png', fullPage: true });

  // Disposal
  await page.getByRole('button', { name: 'Dispose fixture' }).click();
  await page.evaluate(() => window.__FE06__.settled());
  await page.waitForTimeout(200);

  assert.equal(await page.locator('canvas').count(), 0);
  assert.equal((await state()).textures.length, 0);
  assert.equal((await state()).tilemaps, 0);
  assert.deepEqual(await page.evaluate(() => window.__FE06_RESOURCES__()), { urls: 0, frames: 0 });

  await context.close();

  const result = {
    status: 'PASS',
    browser: await browser.version(),
    viewport: '390x844',
    singleCanvasPreserved: true,
    keyboardRadioSelection: true,
    rollbackOnFailure: true,
    cleanDisposal: true,
    pageErrors: errors
  };

  await writeFile('/tmp/mcr-qa-fe06-agy8u3i5/qa-browser-result.json', JSON.stringify(result, null, 2) + '\n');
  console.log('QA Native Browser Result:', JSON.stringify(result, null, 2));
} finally {
  await browser?.close();
  await dev.close();
}

