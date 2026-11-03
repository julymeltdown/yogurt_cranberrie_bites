import assert from 'node:assert/strict';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { resolve, relative } from 'node:path';
import { build, preview } from 'vite';
import react from '@vitejs/plugin-react';
import { chromium, expect } from '@playwright/test';
const root = resolve(import.meta.dirname, '../../../../../..');
const frontend = resolve(root, 'frontend'), out = process.env.MCR_FE37_EVIDENCE ?? '/tmp/mcr-fe37-browser';
await mkdir(out, { recursive: true });
const paths = (await readdir(resolve(frontend, 'src/features/admin'), { recursive: true, withFileTypes: true })).filter(e => e.isFile()).map(e => resolve(e.parentPath, e.name)).sort();
const report = { result: 'FAIL', scope: 'FE37 MOCK_READY only: deterministic candidate fetches, canonical Auth/privacy and native desktop Chrome; no real server authorization, moderation, multiuser or physical-device acceptance.', base: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), browser: null, widths: [], checks: [], sourceHashes: Object.fromEntries(await Promise.all(paths.map(async (p) => [relative(root, p), createHash('sha256').update(await readFile(p)).digest('hex')]))) };
let server, browser;
const actionFor = lane => lane === 'reports' ? 'RESOLVED' : lane === 'members' ? 'SUSPEND' : 'HIDE';
async function prepare(page, lane, action = actionFor(lane)) {
    await page.getByRole('button', { name: /^Review 0000/ }).click();
    await expect(page.getByRole('heading', { name: /^Review / })).toBeFocused();
    await page.getByLabel('Action', { exact: true }).selectOption(action);
    await page.getByRole('textbox', { name: 'Reason for this action' }).fill('Reviewed factual fixture reason');
}
async function confirmByKeyboard(page) {
    await page.getByRole('textbox', { name: 'Reason for this action' }).focus();
    await page.keyboard.press('Tab');
    await expect(page.getByRole('button', { name: 'Review action', exact: true })).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(page.getByRole('button', { name: 'Confirm action', exact: true })).toBeFocused();
}
async function geometry(page) {
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Horizontal overflow');
    const bad = await page.locator('button:enabled,select:enabled,textarea:enabled').evaluateAll(els => els.filter(e => {
        const r = e.getBoundingClientRect();
        return r.width < 44 || r.height < 44;
    }).map(e => ({ text: e.textContent, width: e.getBoundingClientRect().width, height: e.getBoundingClientRect().height })));
    assert.deepEqual(bad, [], 'Controls below 44px');
    assert.equal(await page.getByText('PRIVATE_DIAGNOSTIC', { exact: false }).count(), 0);
    assert.equal(await page.getByText('00000000-0000-4000-8000-000000000999', { exact: false }).count(), 0);
}
try {
    await build({ root: frontend, configFile: false, plugins: [react()], logLevel: 'warn', build: { outDir: resolve(out, 'site'), emptyOutDir: true, target: 'es2022', rollupOptions: { input: resolve(import.meta.dirname, 'fixtures/preview.html') } } });
    server = await preview({ root: frontend, configFile: false, logLevel: 'error', build: { outDir: resolve(out, 'site') }, preview: { host: '127.0.0.1', port: 5199, strictPort: true } });
    browser = await chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true });
    report.browser = browser.version();
    for (const width of [360, 390, 430]) {
        for (const lane of ['reports', 'members', 'content']) {
            const page = await browser.newPage({ viewport: { width, height: 900 } }), errors = [];
            page.on('pageerror', e => errors.push(e.message));
            const url = `http://127.0.0.1:5199/src/features/admin/reports/__tests__/fixtures/preview.html?lane=${lane}`;
            await page.goto(url);
            await prepare(page, lane);
            if (lane === 'reports') {
                await expect(page.locator('img')).toHaveCount(0);
                await page.getByRole('button', { name: 'Open private evidence 1' }).click();
                await expect(page.getByRole('img', { name: 'Private report evidence' })).toHaveAttribute('src', /^blob:/);
                await expect.poll(() => page.locator('img').evaluate(image => image.naturalWidth)).toBeGreaterThan(0);
            }
            await confirmByKeyboard(page);
            await geometry(page);
            await page.screenshot({ path: resolve(out, `${width}-${lane}-confirmation.png`), fullPage: true });
            await page.keyboard.press('Tab');
            await expect(page.getByRole('button', { name: 'Cancel confirmation' })).toBeFocused();
            await page.keyboard.press('Enter');
            await expect(page.getByRole('button', { name: 'Review action', exact: true })).toBeFocused();
            assert.equal(await page.evaluate(() => window.__FE37__.writes()), 0);
            await page.evaluate(() => window.__FE37__.mode('pending'));
            await confirmByKeyboard(page);
            await page.keyboard.press('Enter');
            await expect(page.getByRole('status')).toContainText('Action pending');
            await expect(page.getByRole('button', { name: 'Refresh records' })).toBeDisabled();
            await expect(page.getByRole('button', { name: 'Review action', exact: true })).toBeDisabled();
            assert.equal(await page.evaluate(() => window.__FE37__.writes()), 1);
            await geometry(page);
            await page.screenshot({ path: resolve(out, `${width}-${lane}-pending.png`), fullPage: true });
            await page.getByRole('button', { name: 'Check original command' }).click();
            await expect(page.getByRole('alert')).toContainText('server recorded failure');
            await expect(page.getByRole('textbox', { name: 'Reason for this action' })).toHaveValue('Reviewed factual fixture reason');
            assert.equal(await page.evaluate(() => window.__FE37__.writes()), 1);
            await page.screenshot({ path: resolve(out, `${width}-${lane}-failed.png`), fullPage: true });
            await page.getByRole('button', { name: 'Refresh records' }).click();
            await prepare(page, lane);
            await page.evaluate(() => window.__FE37__.mode('unknown'));
            await confirmByKeyboard(page);
            await page.keyboard.press('Enter');
            await expect(page.getByRole('alert')).toContainText('Outcome unknown');
            await geometry(page);
            await page.screenshot({ path: resolve(out, `${width}-${lane}-unknown.png`), fullPage: true });
            await page.getByRole('button', { name: 'Check original command' }).click();
            await expect(page.getByRole('alert')).toContainText('server recorded failure');
            assert.equal(await page.evaluate(() => window.__FE37__.writes()), 2);
            await page.getByRole('button', { name: 'Refresh records' }).click();
            await prepare(page, lane);
            await page.evaluate(() => window.__FE37__.mode('success'));
            await confirmByKeyboard(page);
            await page.keyboard.press('Enter');
            await expect(page.getByRole('status')).toContainText('server confirmed this action');
            await geometry(page);
            await page.screenshot({ path: resolve(out, `${width}-${lane}-applied.png`), fullPage: true });
            if (lane !== 'reports') {
                await page.getByRole('button', { name: 'Refresh records' }).click();
                await prepare(page, lane, lane === 'members' ? 'UNSUSPEND' : 'RESTORE');
                await confirmByKeyboard(page);
                await page.keyboard.press('Enter');
                await expect(page.getByRole('status')).toContainText('server confirmed this action');
            }
            await page.evaluate(() => window.__FE37__.withdraw());
            await expect(page.getByRole('heading', { name: 'Moderator access unavailable' })).toBeVisible();
            await expect(page.getByRole('textbox')).toHaveCount(0);
            await expect(page.getByRole('img')).toHaveCount(0);
            await expect(page.getByRole('table')).toHaveCount(0);
            await page.goto(url + '&denied=1');
            await expect(page.getByRole('heading', { name: 'Moderator access unavailable' })).toBeVisible();
            await expect(page.getByRole('table')).toHaveCount(0);
            assert.deepEqual(errors, []);
            report.widths.push({ width, lane, result: 'PASS' });
            await page.close();
        }
    }
    report.checks = ['three lanes at360/390/430: no overflow and controls>=44px', 'keyboard review/confirmation/cancel focus and zero mutation on cancel', 'private Blob evidence on demand; reporter identity excluded', 'pending and unknown are not applied; only original GET status; no replay', 'failed receipt preserves factual reason until explicit refresh', 'direct200 revision-based apply and reverse member/content actions', 'permission withdrawal removes evidence, reason, tables and confirmation; denied entry has no private table', 'no browser exceptions or private diagnostic text'];
    report.result = 'PASS';
}
catch (error) {
    report.error = String(error);
    throw error;
}
finally {
    await browser?.close();
    await new Promise(resolveClose => server ? server.httpServer.close(resolveClose) : resolveClose());
    await writeFile(resolve(out, 'report.json'), JSON.stringify(report, null, 2) + '\n');
}
console.log(JSON.stringify({ result: report.result, browser: report.browser, widths: report.widths, report: resolve(out, 'report.json') }));
