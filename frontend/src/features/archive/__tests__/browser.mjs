import assert from 'node:assert/strict';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { resolve, relative } from 'node:path';
import { build, preview } from 'vite';
import react from '@vitejs/plugin-react';
import { chromium, expect } from '@playwright/test';
const root = resolve(import.meta.dirname, '../../../../..'), feature = resolve(import.meta.dirname, '..'), frontend = resolve(root, 'frontend'), out = process.env.MCR_FE40_EVIDENCE ?? '/tmp/mcr-fe40-browser';
await mkdir(out, { recursive: true });
const source = (await readdir(feature, { recursive: true, withFileTypes: true })).filter(e => e.isFile() && /\.(?:ts|tsx|mjs|html|css)$/.test(e.name)).map(e => resolve(e.parentPath, e.name));
source.push(resolve(root, 'contracts/generated/mcr-domain.openapi.json'), resolve(root, 'contracts/domain/v1/admin.schema.json'), resolve(root, 'contracts/domain/v1/mural.schema.json'), resolve(root, 'contracts/domain/v1/operations.json'), resolve(frontend, 'src/features/event-feed/contract.ts'));
const digest = async (path) => createHash('sha256').update(await readFile(path)).digest('hex');
const report = { result: 'FAIL', scope: 'FE40/X01 MOCK_READY: deterministic registered public HTTP fixtures, mobile touch contexts in native desktop Chrome. BE41/live archive, qualified public media and physical iOS/Android NOT_RUN.', assignmentBase: 'cd7192febf9c2764169b55d63a70d7e00d13fd08', testedHead: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), browser: null, configurations: [], checks: [], screenshots: [], sourceHashes: Object.fromEntries(await Promise.all(source.sort().map(async (p) => [relative(root, p), await digest(p)]))) };
let browser, server;
async function geometry(page) {
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'horizontal overflow');
    assert.deepEqual(await page.locator('button:enabled').evaluateAll(nodes => nodes.filter(n => {
        const r = n.getBoundingClientRect();
        return r.width < 44 || r.height < 44;
    }).map(n => n.textContent)), []);
    await expect(page.locator('form,input,textarea,select,a[href*="/feed"],a[href*="/thread"]')).toHaveCount(0);
}
async function capture(page, width, state) {
    const file = `${width}-${state}.png`;
    await geometry(page);
    await page.screenshot({ path: resolve(out, file), fullPage: true });
    report.screenshots.push({ file, width, state, sha256: await digest(resolve(out, file)) });
}
try {
    await build({ root: frontend, configFile: false, plugins: [react()], logLevel: 'warn', build: { outDir: resolve(out, 'site'), emptyOutDir: true, target: 'es2022', rollupOptions: { input: resolve(import.meta.dirname, 'fixtures/preview.html') } } });
    server = await preview({ root: frontend, configFile: false, logLevel: 'error', build: { outDir: resolve(out, 'site') }, preview: { host: '127.0.0.1', port: 5201, strictPort: true } });
    browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true });
    report.browser = browser.version();
    for (const width of [360, 390, 430]) {
        const page = await browser.newPage({ viewport: { width, height: 844 }, isMobile: true, hasTouch: true, reducedMotion: 'reduce' }), errors = [];
        page.on('pageerror', e => errors.push(e.message));
        await page.goto('http://127.0.0.1:5201/src/features/archive/__tests__/fixtures/preview.html');
        await expect(page.getByRole('heading', { name: 'Posts after the show' })).toBeFocused();
        await expect(page.getByText('We sang every word.', { exact: false })).toBeVisible();
        assert.deepEqual(await page.locator('canvas').evaluate(canvas => Array.from(canvas.getContext('2d').getImageData(1, 1, 1, 1).data)), [21, 23, 28, 255]);
        assert.deepEqual(await page.locator('canvas').evaluate(canvas => Array.from(canvas.getContext('2d').getImageData(127, 127, 1, 1).data)), [150, 157, 163, 255]);
        await page.locator('canvas').tap({ position: { x: 30, y: 30 } });
        await page.getByRole('button', { name: /View archived image 1/ }).tap();
        await expect.poll(() => page.locator('img').evaluate(img => img.naturalWidth)).toBeGreaterThan(0);
        await capture(page, width, 'ready');
        await page.getByRole('button', { name: 'Next page' }).focus();
        await page.keyboard.press('Enter');
        await expect(page.getByText('The last chorus stayed with us.')).toBeVisible();
        await expect(page.getByText('We sang every word.', { exact: false })).toHaveCount(0);
        await expect(page.getByRole('button', { name: 'Next page' })).toHaveCount(0);
        await capture(page, width, 'next-page');
        await page.evaluate(() => window.__FE40__.account());
        await expect(page.locator('canvas')).toHaveCount(0);
        await page.getByRole('button', { name: 'Reload archive' }).tap();
        await expect(page.locator('canvas')).toBeVisible();
        await page.evaluate(() => window.__FE40__.invalidate());
        await expect(page.getByRole('alert')).toContainText('content was removed');
        await expect(page.locator('canvas')).toHaveCount(0);
        await page.evaluate(() => window.__FE40__.mode('private'));
        await page.getByRole('button', { name: 'Reload archive' }).tap();
        await expect(page.getByRole('alert')).toContainText('could not be verified');
        assert(!(await page.evaluate(() => window.__FE40__.state())).includes('PRIVATE_DIAGNOSTIC'));
        await page.evaluate(() => window.__FE40__.mode('unavailable'));
        await page.getByRole('button', { name: 'Reload archive' }).tap();
        await expect(page.getByRole('alert')).toContainText('archive is unavailable');
        await capture(page, width, 'unavailable');
        await page.evaluate(() => window.__FE40__.mode('empty'));
        await page.getByRole('button', { name: 'Reload archive' }).tap();
        await expect(page.getByText('No permitted posts on this page.')).toBeVisible();
        await capture(page, width, 'empty');
        await page.evaluate(() => window.__FE40__.mode('offline'));
        await page.getByRole('button', { name: 'Reload archive' }).tap();
        await expect(page.getByRole('alert')).toContainText('could not be reached');
        await capture(page, width, 'offline');
        await page.evaluate(() => {
            window.__FE40__.mode('image-unavailable');
            window.__FE40__.event('00000000-0000-4000-8000-000000000002');
        });
        await page.getByRole('button', { name: 'Reload archive' }).tap();
        await expect(page.locator('canvas')).toBeVisible();
        await page.getByRole('button', { name: /View archived image 1/ }).tap();
        await expect(page.getByRole('status')).toContainText('Archived image unavailable');
        await page.evaluate(() => window.__FE40__.event(null));
        await expect(page.locator('canvas,img,article')).toHaveCount(0);
        const requests = await page.evaluate(() => window.__FE40__.requests());
        assert(requests.length > 5);
        assert(requests.every(r => r.method === 'GET' && r.credentials === 'omit' && r.cache === 'no-store' && !r.authorization && /\/archive(?:\?|$)/.test(r.url)));
        assert.deepEqual(errors, []);
        report.configurations.push({ width, height: 844, isMobile: true, hasTouch: true, reducedMotion: 'reduce', result: 'PASS' });
        await page.close();
    }
    report.checks = ['native Chrome mobile touch360/390/430 and keyboard pagination', 'decoded Blob image and exact frozen canvas pixel samples', 'all requests are registered archive GET,omit credentials,no-store,no auth,zero mutations', 'no horizontal overflow; enabled controls>=44px; no live-feed links/forms/write inputs', 'one-page pagination and focus; previous/deleted posts disappear', 'account/event/invalidation remove post/image/canvas data', 'strict private/deleted projection is rejected and never rendered/retained', 'unavailable/empty/offline and missing public image states', 'no browser exceptions'];
    report.result = 'PASS';
}
catch (error) {
    report.error = String(error);
    throw error;
}
finally {
    await browser?.close();
    await new Promise(done => server ? server.httpServer.close(done) : done());
    await writeFile(resolve(out, 'report.json'), JSON.stringify(report, null, 2) + '\n');
}
console.log(JSON.stringify({ result: report.result, browser: report.browser, configurations: report.configurations, report: resolve(out, 'report.json') }));
