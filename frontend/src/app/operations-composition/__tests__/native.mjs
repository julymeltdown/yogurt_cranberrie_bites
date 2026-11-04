// One changed default-main Admin journey. Every authority/HTTP value is TEST ONLY.
import assert from 'node:assert/strict'
import { readFile, writeFile, mkdir } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { createServer } from 'vite'
import { chromium, expect } from '@playwright/test'
import { createControlledServer, wire } from './server.ts'
const root = resolve(import.meta.dirname, '../../../../..'), out = process.env.ADMIN_OUTPUT
if (!out || !process.env.ADMIN_PORT || !process.env.ADMIN_TMP) throw Error('Fresh bounded offer environment required')
await mkdir(out, { recursive: true })
const registration = JSON.parse(await readFile(resolve(root, 'frontend/src/app/operations-entry/__tests__/operation.fixture.json')))
const controlled = createControlledServer()
const report = { result: 'FAIL', source: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  scope: 'Unmodified actual index.html/main.tsx/default AdminBoundary/production ports, one390 context. Controlled HTTP and explicit TEST_ONLY frozen registration injected only by this harness. No real205 producer/default live role grant or backend/liveIN04/IN05 acceptance.',
  viewport: { width: 390, height: 844 }, sourceHashes: {}, requests: [], errors: [], captures: [] }
for (const path of ['frontend/index.html','frontend/src/main.tsx','frontend/src/app/AdminBoundary.tsx','frontend/src/app/ports.ts',
  'frontend/src/app/operations-composition/owner.ts','frontend/src/app/operations-composition/panel.tsx',
  'frontend/src/app/operations-composition/__tests__/server.ts','frontend/src/app/operations-composition/__tests__/wire.json',
  'frontend/src/app/operations-entry/index.ts','frontend/src/shared/api/domain-operations.generated.ts']) report.sourceHashes[path] = createHash('sha256').update(await readFile(resolve(root,path))).digest('hex')
const server = await createServer({ root: resolve(root,'frontend'), configFile: false, logLevel: 'error', cacheDir: resolve(process.env.ADMIN_TMP,'vite-cache'),
  define: { 'import.meta.env.VITE_MCR_EVENT_ID': JSON.stringify(wire.event) },
  plugins: [{ name: 'TEST_ONLY-frozen-operations-registration', enforce: 'pre', transform(code,id) {
    if (!id.endsWith('/shared/api/domain-operations.generated.ts')) return
    return { code: `${code}\nObject.assign(DOMAIN_OPERATIONS, ${JSON.stringify({ [registration.operationId]: registration.definition })});`, map: null }
  } }],
  server: { host: '127.0.0.1', port: Number(process.env.ADMIN_PORT), strictPort: true },
  optimizeDeps: { include: ['react','react-dom/client','react/jsx-runtime','react-router-dom','@tanstack/react-query','zod','ky'] } })
let browser
const cleanup = { browserClosed: false, serverClosed: false, errors: [] }
try {
  await server.listen()
  browser = await chromium.launch({ executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true,args:['--disable-dev-shm-usage'] })
  report.browser = browser.version()
  const context = await browser.newContext({ viewport:report.viewport,hasTouch:true,isMobile:true,deviceScaleFactor:1,reducedMotion:'reduce' })
  const page = await context.newPage();page.on('pageerror',e=>report.errors.push(e.message))
  await page.route('**/api/**',async route=>{
    const input=route.request()
    try {
      const response=await controlled.handle(new Request(input.url(),{method:input.method(),headers:input.headers(),...(input.postData() ? {body:input.postData()} : {})}))
      await route.fulfill({status:response.status,headers:Object.fromEntries(response.headers),body:await response.text()})
    } catch { await route.abort('failed') }
  })
  await page.goto(`http://127.0.0.1:${process.env.ADMIN_PORT}/login`)
  await page.getByLabel('Email',{exact:true}).fill('operator@example.com')
  await page.getByLabel('Password',{exact:true}).fill('TEST_ONLY_default_admin_password')
  await page.getByRole('button',{name:'Sign in with email'}).click()
  await page.waitForURL('**/world')
  // Admin is deliberately excluded from sign-in return URLs. Use normal browser
  // history navigation in the already signed-in SPA; never inject an identity.
  await page.evaluate(()=>{history.pushState(null,'','/admin/mural');dispatchEvent(new PopStateEvent('popstate'))})
  const status=page.getByRole('status',{name:'Wall control status'})
  await expect(status).toContainText('Server wall state loaded')
  await page.getByLabel('Reason',{exact:true}).fill('Reviewed default Admin retained command')
  await page.getByRole('button',{name:'Review wall action',exact:true}).click()
  await expect(page.getByRole('group',{name:'Confirm wall action'})).toBeFocused()
  controlled.state.lost=true
  await page.getByRole('button',{name:'Confirm wall action',exact:true}).click()
  await expect(status).toContainText('unresolved')
  const writes=()=>controlled.requests.filter(r=>r.method==='POST'&&new URL(r.url).pathname.endsWith('/control'))
  assert.equal(writes().length,1);const originalKey=writes()[0].headers.get('Idempotency-Key')
  async function capture(name) { await page.screenshot({fullPage:true,path:resolve(out,name)});report.captures.push(name) }
  await capture('390-retained-unknown.png')
  await page.getByRole('link',{name:'Event',exact:true}).click()
  await expect(page.getByRole('heading',{name:'Event settings'})).toBeVisible()
  await expect(page.getByRole('heading',{name:wire.settings.event.title,exact:true})).toBeVisible()
  await capture('390-event.png')
  await page.getByRole('link',{name:'Places',exact:true}).click()
  await expect(page.getByRole('heading',{name:'Venue and places'})).toBeVisible()
  await page.getByRole('link',{name:'Closure',exact:true}).click()
  await expect(page.getByRole('heading',{name:'CLOSE THE EVENT'})).toBeVisible()
  await capture('390-closure.png')
  await page.getByRole('link',{name:'Wall',exact:true}).click()
  await expect(status).toContainText('unresolved');assert.equal(writes().length,1)
  assert.equal(writes()[0].headers.get('Idempotency-Key'),originalKey)
  controlled.state.lost=false
  await page.getByRole('button',{name:'Check original wall command'}).click()
  await expect(status).toContainText('succeeded');assert.equal(writes().length,1)
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true)
  controlled.state.role=null;controlled.replaceDecision()
  await page.getByRole('button',{name:'Check current access'}).click()
  await expect(page.getByRole('heading',{name:'Administration unavailable'})).toBeVisible()
  await expect(page.getByRole('heading',{name:'WALL CONTROL'})).toHaveCount(0)
  await expect(page.getByText('PRIVATE diagnostic must never render')).toHaveCount(0)
  await capture('390-withdrawn.png')
  report.requests=controlled.requests.map(r=>({method:r.method,path:new URL(r.url).pathname,key:r.headers.get('Idempotency-Key')}))
  const admission=report.requests.findIndex(r=>r.path==='/api/auth/enrollment/admission')
  const role=report.requests.findIndex(r=>r.path.endsWith('/me/operations-access'))
  assert(admission>=0&&role>admission,'Canonical registered admission must precede Admin role HTTP')
  assert.equal(controlled.requests.filter(r=>new URL(r.url).pathname==='/api/auth/login').length,1)
  assert(controlled.requests.filter(r=>new URL(r.url).pathname.includes('/admin/')).every(r=>r.headers.get('Authorization')===`Bearer ${wire.auth.accessToken}`))
  report.canonicalAdmissionBeforeAdmin=true;report.identityInjected=false
  assert.deepEqual(report.errors,[]);report.result='PASS';report.retainedOriginalKey=originalKey;report.singleMutation=true;report.withdrawal=true
  await context.close()
} catch(error) {report.failure=String(error);process.exitCode=1}
finally {
  await writeFile(resolve(out,'native.json'),JSON.stringify(report,null,2))
  try {await browser?.close();cleanup.browserClosed=true}catch(error){cleanup.errors.push(String(error))}
  try {await server.close();cleanup.serverClosed=true}catch(error){cleanup.errors.push(String(error))}
  await writeFile(resolve(out,'cleanup.json'),JSON.stringify(cleanup,null,2));if(cleanup.errors.length)process.exitCode=1
}
