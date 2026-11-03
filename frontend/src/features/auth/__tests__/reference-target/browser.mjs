import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile, readdir, rm, statfs } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { createServer } from 'vite'
import react from '@vitejs/plugin-react'
import { chromium, expect } from '@playwright/test'
const own = 'src/features/auth/__tests__/reference-target', evidence = `${own}/evidence`
const fs = await statfs('.'); assert(fs.bavail * fs.bsize > 8 * 1024 ** 3, 'disk must exceed 8GiB')
await mkdir(evidence, { recursive: true })
const dev = await createServer({ configFile: false, plugins: [react()], cacheDir: `${own}/.cache/browser`, server: { host: '127.0.0.1', port: 0 } })
let browser
const errors = [], results = [], captures = []
const hash = value => createHash('sha256').update(value).digest('hex')
try {
  await dev.listen(); const base = `http://127.0.0.1:${dev.httpServer.address().port}/src/features/auth/__tests__/reference-target/preview.html`
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const width of [360,390,430]) {
    const context = await browser.newContext({ viewport: { width, height: Math.round(width * 520 / 280) + 26 }, reducedMotion: 'reduce' })
    const page = await context.newPage(); page.on('pageerror', e => errors.push(e.message))
    const open = async (query = '') => { await page.goto(base + query); await expect(page.getByRole('heading', { name: 'Sign in', exact: true })).toBeVisible() }
    const fill = async (password = 'wrong') => { await expect(page.getByRole('button', { name: 'Sign in with email' })).toBeEnabled(); await page.getByLabel('Email', { exact: true }).fill('synthetic@example.com'); await page.getByLabel('Password', { exact: true }).fill(password) }
    const capture = async name => { const path = `${evidence}/${name}-${width}.png`; await page.screenshot({ path, fullPage: true }); captures.push(path) }
    await open(); await fill()
    for (const provider of ['X','Discord','Google']) { await expect(page.getByRole('button', { name: `Continue with ${provider}` })).toBeDisabled(); await expect(page.getByRole('button', { name: `Continue with ${provider}` }).locator('svg')).toBeVisible() }
    assert.equal(await page.getByRole('checkbox').count(), 0)
    await page.getByLabel('Email', { exact: true }).focus(); await page.keyboard.press('Tab'); await expect(page.getByLabel('Password', { exact: true })).toBeFocused()
    assert.notEqual(await page.getByLabel('Password', { exact: true }).evaluate(el => getComputedStyle(el).outlineStyle), 'none')
    const geometry = await page.evaluate(() => {
      const box = selector => { const r = document.querySelector(selector).getBoundingClientRect(); return { x:r.x, y:r.y, width:r.width, height:r.height } }
      return { frame: box('.concert-window'), heading:box('.auth-display-heading'), email:box('#sign-in-email'), password:box('#sign-in-password'), submit:box('.auth-submit'), providers:box('.auth-provider-buttons'), footer:box('.auth-create'), overflow:document.documentElement.scrollWidth > innerWidth }
    })
    assert.equal(geometry.overflow,false)
    assert(geometry.email.width / geometry.frame.width >= .72 && geometry.email.width / geometry.frame.width <= .82, 'reference narrow form geometry')
    assert.equal(geometry.submit.width,geometry.email.width)
    for (const control of await page.locator('.auth-sign-in input,.auth-sign-in button,.auth-sign-in a').all()) { const r = await control.boundingBox(); assert(r && r.width >= 44 && r.height >= 44, '44px interactive target') }
    await capture('ready')
    await page.getByRole('button', { name: 'Sign in with email' }).click()
    await expect(page.getByText('We could not sign you in. Check your details or try again later.')).toBeVisible()
    await expect(page.getByLabel('Password', { exact: true })).toHaveValue('')
    assert.equal(await page.evaluate(() => window.__REF_AUTH__.count()),1)
    assert(!(await page.locator('body').innerText()).includes('PRIVATE_RAW_ERROR'))
    if(width===390) await capture('credentials-error')
    await fill('throttle'); await page.getByRole('button', { name:'Sign in with email' }).click(); await expect(page.getByText('Too many attempts. Wait a while before trying again.')).toBeVisible()
    await fill('secret'); await page.evaluate(() => window.__REF_AUTH__.invalidate()); await expect(page.getByLabel('Email', { exact:true })).toHaveValue(''); await expect(page.getByLabel('Password', { exact:true })).toHaveValue('')
    await open('?mode=pending'); await fill('secret'); await page.getByRole('button', { name:'Sign in with email' }).click(); await expect(page.getByRole('button', { name:'Signing in…' })).toBeDisabled(); await expect(page.getByLabel('Password', { exact:true })).toHaveValue('')
    await page.locator('form').evaluate(el => { el.dispatchEvent(new Event('submit',{ bubbles:true,cancelable:true })) }); assert.equal(await page.evaluate(() => window.__REF_AUTH__.count()),1)
    if(width===390) await capture('pending')
    await page.getByRole('button', { name:'Close sign-in and return home' }).click(); await expect.poll(() => page.evaluate(() => window.__REF_AUTH__.path())).toBe('/'); await page.evaluate(() => window.__REF_AUTH__.settle()); await page.waitForTimeout(25); assert.equal(await page.evaluate(() => window.__REF_AUTH__.path()),'/')
    await open('?mode=options-error'); await expect(page.getByRole('button', { name:'Check options again' })).toBeVisible(); await expect(page.getByRole('button', { name:'Sign in with email' })).toBeDisabled()
    await open('?mode=options-loading'); await expect(page.getByText('Checking sign-in options…')).toBeVisible(); await expect(page.getByRole('button', { name:'Sign in with email' })).toBeDisabled()
    await open('?mode=unavailable'); await expect(page.getByText('Event entry is not open yet. Please check back later.')).toBeVisible(); assert.equal(await page.getByRole('form').count(),0)
    if(width===390) await capture('unavailable')
    await open('?target=https%3A%2F%2Fforeign.invalid'); await fill('secret'); await page.getByRole('button', { name:'Sign in with email' }).click(); await expect.poll(() => page.evaluate(() => window.__REF_AUTH__.path())).toBe('/world')
    assert.equal(await page.evaluate(() => localStorage.length + sessionStorage.length),0)
    await page.evaluate(() => window.__REF_AUTH__.dispose()); await context.close()
    results.push({width, geometry, fieldsProvidersKeyboard:'PASS', credentialsThrottleScrub:'PASS', pendingSingleSubmitUnmount:'PASS', unavailableOptions:'PASS', safeRedirectNoStorage:'PASS'})
  }
  assert.deepEqual(errors,[])
  const sources = {}
  async function scan(dir) { for(const e of await readdir(dir,{withFileTypes:true})) { if(e.name==='evidence'||e.name==='.cache') continue; const p=`${dir}/${e.name}`; if(e.isDirectory()) await scan(p); else if(/\.(tsx?|css|mjs|html)$/.test(e.name)) sources[`frontend/${p}`]=hash(await readFile(p)) } }
  await scan('src/features/auth/ui/reference-sign-in'); await scan(own)
  for(const p of ['src/features/auth/ui/SignIn.tsx','src/features/auth/ui/sign-in.css','src/shared/ui/concert-window/index.tsx','src/shared/ui/concert-window/concert-window.css','src/features/auth/model/session.ts','src/features/auth/__tests__/sign-in.test.tsx']) sources[`frontend/${p}`]=hash(await readFile(p))
  await writeFile(`${evidence}/report.json`,JSON.stringify({status:'MOCK_READY',browser:browser.version(),referenceSha256:'d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617',referenceComparison:'Screen 2 proportions: narrow centered stacked fields, equal-width red submit, short divider/provider row, cream/black cemetery silhouette. Reference remember-session deliberately omitted; all providers unavailable; no invented wire.',results,errors,sources,captures:Object.fromEntries(await Promise.all(captures.map(async p=>[`frontend/${p}`,hash(await readFile(p))]))),limitations:'Production SignIn with canonical Auth and deterministic synthetic fetch/cookie policy. No actual backend/provider/mobile device/root ten-route acceptance.'},null,2)+'\n')
  console.log(JSON.stringify({results:results.map(x=>({width:x.width,result:'PASS'})),errors}))
} finally { await browser?.close(); await dev.close(); await rm(`${own}/.cache/browser`,{recursive:true,force:true}) }
