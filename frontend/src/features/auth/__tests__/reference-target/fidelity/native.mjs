import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { readFile, writeFile, mkdir, rm, statfs } from 'node:fs/promises'
import { resolve, relative } from 'node:path'
import { createServer } from 'vite'
import react from '@vitejs/plugin-react'
import { chromium, expect } from '@playwright/test'
const own = 'src/features/auth/__tests__/reference-target/fidelity', output = `${own}/evidence`, root = resolve('..')
const disk = await statfs('.'); assert(disk.bavail * disk.bsize > 8 * 1024 ** 3)
await mkdir(output, { recursive: true })
const hash = v => createHash('sha256').update(v).digest('hex')
const target = await readFile(`${root}/docs/mcr/design/reference-target-2026-11-04.png`)
assert.equal(hash(target), 'd9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617')
const server = await createServer({ configFile: false, logLevel: 'warn', plugins: [react(), { name: 'test-only-actual-route-entry', configureServer(s) { s.middlewares.use((req,_res,next) => { if (/^\/(login|signup|reset-password|world|gifts|\?)/.test(req.url) || req.url === '/') req.url = `/${own}/app.html`; next() }) } }], cacheDir: `${own}/.cache/vite`, server: { host: '127.0.0.1', port: 0 } })
let browser
const errors = [], results = [], captures = []
try {
  await server.listen(); const base = `http://127.0.0.1:${server.httpServer.address().port}`
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
  for (const width of [360,390,430]) {
    const context = await browser.newContext({ viewport: { width, height: Math.round(width * 526 / 275) }, hasTouch: true, reducedMotion: 'reduce' }), page = await context.newPage()
    page.on('pageerror', e => errors.push(e.message))
    const open = async (query='') => { await page.goto(`${base}/login${query}`); await expect(page.getByRole('heading',{name:'Sign in',exact:true})).toBeVisible(); await expect.poll(()=>page.evaluate(()=>Boolean(window.__AUTH_ROUTE__))).toBe(true) }
    const fill = async password => { await expect(page.getByRole('button',{name:'Sign in with email'})).toBeEnabled(); await page.getByLabel('Email',{exact:true}).fill('fixture@example.test'); await page.getByLabel('Password',{exact:true}).fill(password) }
    const capture = async name => { const path=`${output}/${name}-${width}.png`; await page.screenshot({path,fullPage:true}); captures.push(path) }
    await open(); await expect(page.getByRole('button',{name:'Sign in with email'})).toBeEnabled(); await page.evaluate(()=>document.fonts.ready)
    const geometry = await page.evaluate(()=>{
      const box=s=>{const r=document.querySelector(s).getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height}}
      return {frame:box('.concert-window'),heading:box('.auth-display-heading'),email:box('#sign-in-email'),password:box('#sign-in-password'),submit:box('.auth-submit'),providers:box('.auth-provider-buttons'),create:box('.auth-create'),overflow:document.documentElement.scrollWidth>innerWidth,scrollHeight:document.documentElement.scrollHeight,viewportHeight:innerHeight}
    })
    assert(!geometry.overflow); assert(geometry.frame.width / width > .94); assert(geometry.email.width / geometry.frame.width > .75 && geometry.email.width / geometry.frame.width < .82)
    for(const c of await page.locator('.auth-sign-in input,.auth-sign-in button,.auth-sign-in a').all()){const r=await c.boundingBox();assert(r.width>=44&&r.height>=44)}
    // Small foreground graves stay below the final text/control row; large side crosses remain outside it.
    const art = await page.evaluate(()=>({art:document.querySelector('.auth-cemetery--bottom').getBoundingClientRect().toJSON(),content:document.querySelector('.auth-paper-content').getBoundingClientRect().toJSON()}))
    assert(art.art.y + art.art.height * 77 / 120 >= art.content.bottom - 2, `footer scenery must not obscure the create-account row: ${JSON.stringify(art)}`)
    await capture('login')
    for(const p of ['X','Discord','Google']) await expect(page.getByRole('button',{name:`Continue with ${p}`})).toBeDisabled()
    assert.equal(await page.getByRole('checkbox').count(),0)
    await page.getByLabel('Email',{exact:true}).focus(); await page.keyboard.press('Tab'); await expect(page.getByLabel('Password',{exact:true})).toBeFocused(); assert.notEqual(await page.getByLabel('Password',{exact:true}).evaluate(e=>getComputedStyle(e).outlineStyle),'none')
    // Native form validity prevents a request, and normal keyboard submission is real.
    await page.getByRole('button',{name:'Sign in with email'}).click(); assert.equal((await page.evaluate(()=>window.__AUTH_ROUTE__.state())).count,0)
    await fill('wrong'); await page.getByLabel('Password',{exact:true}).press('Enter'); await expect(page.getByText('We could not sign you in. Check your details or try again later.')).toBeVisible(); await expect(page.getByLabel('Password',{exact:true})).toHaveValue(''); assert(!(await page.locator('body').innerText()).includes('PRIVATE_RAW_ERROR'))
    if(width===390) await capture('credentials-error')
    for(const c of await page.locator('.auth-sign-in input,.auth-sign-in button:not(:disabled),.auth-sign-in a').all()) {
      await c.scrollIntoViewIfNeeded(); assert(await c.evaluate(e=>{const r=e.getBoundingClientRect(),hit=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);return hit===e||e.contains(hit)}),'foreground controls remain reachable through scenery')
    }
    await fill('throttle'); await page.getByRole('button',{name:'Sign in with email'}).click(); await expect(page.getByText('Too many attempts. Wait a while before trying again.')).toBeVisible()
    await fill('secret'); await page.evaluate(()=>window.__AUTH_ROUTE__.invalidate()); await expect(page.getByLabel('Email',{exact:true})).toHaveValue(''); await expect(page.getByLabel('Password',{exact:true})).toHaveValue('')
    await open('?fixtureMode=pending'); await fill('secret'); await page.getByRole('button',{name:'Sign in with email'}).click(); await expect(page.getByRole('button',{name:'Signing in…'})).toBeDisabled(); await expect(page.getByLabel('Password',{exact:true})).toHaveValue('')
    await page.locator('form').evaluate(e=>e.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true}))); assert.equal((await page.evaluate(()=>window.__AUTH_ROUTE__.state())).count,1)
    if(width===390) await capture('pending')
    await page.getByRole('button',{name:'Close sign-in and return home'}).click(); await expect.poll(()=>page.evaluate(()=>window.__AUTH_ROUTE__.state().path)).toBe('/'); await page.evaluate(()=>window.__AUTH_ROUTE__.settle()); await expect.poll(()=>page.evaluate(()=>window.__AUTH_ROUTE__.state().readiness)).toBe('ready'); assert.equal((await page.evaluate(()=>window.__AUTH_ROUTE__.state())).path,'/world') // signed-in landing owns this redirect, not the unmounted form
    await open('?fixtureMode=unavailable'); await expect(page.getByText('Event entry is not open yet. Please check back later.')).toBeVisible(); assert.equal(await page.getByRole('form').count(),0); if(width===390)await capture('unavailable')
    await open('?fixtureMode=options-error'); await expect(page.getByRole('button',{name:'Check options again'})).toBeVisible(); await expect(page.getByRole('button',{name:'Sign in with email'})).toBeDisabled()
    await open(); await page.getByRole('link',{name:'Forgot password?'}).click(); await expect.poll(()=>page.evaluate(()=>window.__AUTH_ROUTE__.state().path)).toBe('/reset-password'); await expect(page.getByRole('heading',{name:/Reset password/i})).toBeVisible()
    await open(); await page.getByRole('link',{name:'Create an account'}).click(); await expect.poll(()=>page.evaluate(()=>window.__AUTH_ROUTE__.state().path)).toBe('/signup'); await expect(page.getByRole('heading',{name:/Create account/i})).toBeVisible(); assert.equal(await page.evaluate(()=>getComputedStyle(document.body).paddingTop),'16px','Auth frame padding must release on other routes')
    const gift='/gifts/00000000-0000-4000-8000-00000000003c'
    await open(`?returnTo=${encodeURIComponent(gift)}`); await expect(page.getByRole('link',{name:'Back to your destination'})).toHaveAttribute('href',gift); await fill('secret'); await page.getByRole('button',{name:'Sign in with email'}).click(); await expect.poll(()=>page.evaluate(()=>window.__AUTH_ROUTE__.state().path)).toBe(gift); assert.equal((await page.evaluate(()=>window.__AUTH_ROUTE__.state())).count,1)
    await open('?returnTo=https%3A%2F%2Fforeign.invalid'); await expect(page.getByRole('link',{name:'Back to your destination'})).toHaveAttribute('href','/world')
    assert.equal(await page.evaluate(()=>localStorage.length+sessionStorage.length),0)
    await page.evaluate(()=>window.__AUTH_ROUTE__.dispose()); await context.close()
    results.push({width,geometry,actualBrowserRoutes:'PASS',nativeValidityKeyboard:'PASS',credentialsThrottlePrivacy:'PASS',pendingSingleSubmit:'PASS',safeReturnCanonicalLogin:'PASS',unsupportedProviders:'PASS'})
  }
  assert.deepEqual(errors,[])
  const targetRelative = relative(resolve(output),resolve(root,'docs/mcr/design/reference-target-2026-11-04.png'))
  await writeFile(`${output}/comparison.html`, `<!doctype html><html lang="en"><meta charset="utf-8"><title>Auth actual-route comparison</title><style>body{font:14px Arial;background:#e9e3da;color:#211d19;margin:16px}section{display:flex;gap:18px;align-items:flex-start;margin:24px 0}figure{margin:0}figcaption{margin-bottom:10px}.target{position:relative;overflow:hidden}.target img{position:absolute;max-width:none}.actual{display:block}</style><h1>Auth fidelity · actual browser routes</h1><p>Preserved target at original aspect beside functioning /login with deterministic Auth HTTP. No live service/device acceptance. Remember-session omitted; providers remain unavailable. New cemetery drawing differs from target pixels.</p>${[360,390,430].map(w=>{const scale=w/275;return `<section><figure><figcaption>Preserved reference · ${w}px</figcaption><div class="target" style="width:${w}px;height:${Math.round(w*526/275)}px"><img alt="Preserved Auth reference" src="${targetRelative}" style="width:${1536*scale}px;height:${1024*scale}px;left:${-309*scale}px;top:${-3*scale}px"></div></figure><figure><figcaption>Actual /login · ${w}px · synthetic HTTP</figcaption><img class="actual" alt="Actual functioning login route at ${w}px" width="${w}" src="login-${w}.png"></figure></section>`}).join('')}</html>`)
  const comparisonContext=await browser.newContext({viewport:{width:834,height:870}}), comparisonPage=await comparisonContext.newPage()
  await comparisonPage.goto(`file://${resolve(output,'comparison.html')}`);await comparisonPage.locator('section').nth(1).scrollIntoViewIfNeeded();await comparisonPage.locator('section').nth(1).screenshot({path:`${output}/comparison-390.png`});captures.push(`${output}/comparison-390.png`);await comparisonContext.close()
  const sources={}
  for(const p of ['src/features/auth/ui/SignIn.tsx','src/features/auth/ui/sign-in.css','src/features/auth/ui/reference-sign-in/Cemetery.tsx','src/features/auth/ui/reference-sign-in/ProviderMark.tsx','src/app/router.tsx','src/app/PublicBoundary.tsx','src/app/ports.ts','src/shared/ui/concert-window/concert-window.css','src/shared/styles/base.css','src/features/auth/model/session.ts',`${own}/app.tsx`,`${own}/native.mjs`])sources[`frontend/${p}`]=hash(await readFile(p))
  await writeFile(`${output}/report.json`,JSON.stringify({status:'MOCK_READY',candidate:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),referenceSha256:hash(target),browser:browser.version(),composition:'Actual createAppRouter/createProductionPorts on real /login browser URL; no rendering/composition override; synthetic Auth HTTP/cookie policy; bootstrap deliberately denied',results,errors,sources,captures:Object.fromEntries(await Promise.all(captures.map(async p=>[`frontend/${p}`,hash(await readFile(p))]))),limits:'No live backend/provider, mobile hardware, or world qualification. Visual comparison is developer inspection, not pixel identity or complete ten-route acceptance.'},null,2)+'\n')
  console.log(JSON.stringify({results,errors}))
}finally{await browser?.close();await server.close();await rm(`${own}/.cache`,{recursive:true,force:true})}
