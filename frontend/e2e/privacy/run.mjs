import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile, rm } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createHarnessServer, here, root, requireDependency } from './runtime.mjs'

const output = resolve(root, 'docs/mcr/evidence/privacy-frontend')
const server = await createHarnessServer()
let browser
const errors = []
try {
  await mkdir(output, { recursive: true })
  await server.listen()
  const { chromium } = requireDependency('@playwright/test')
  browser = await chromium.launch({ headless: true, executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' })
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } })
  page.on('pageerror', error => errors.push(error.message)) // Synthetic runner only; no real credentials/payloads.
  // Reject accidental live calls: this runner must remain fixture-only.
  await page.route(url => url.pathname.startsWith('/api/'), route => { errors.push('unexpected network API dispatch'); return route.abort() })
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/`)
  try { await page.waitForFunction(() => window.qa04 !== undefined, null, { timeout: 15000 }) }
  catch { console.error(errors); throw new Error('QA04 fixture module failed to start') }
  const report = await page.evaluate(() => window.qa04)
  assert.deepEqual(errors, [])
  assert.equal(report.result, 'MOCK_READY')
  assert.equal(report.cells.filter(cell => cell.status === 'PASS').length, 28)
  assert.equal(report.cells.filter(cell => cell.status === 'UNSUPPORTED').length, 7)
  // No private payload is rendered by the qualification shell.
  assert.doesNotMatch(await page.locator('body').innerText(), /qa04_private_/)
  const files = [
    'frontend/package.json', 'frontend/pnpm-lock.yaml',
    'contracts/generated/mcr-domain.openapi.json', 'contracts/domain/v1/social.schema.json', 'contracts/domain/v1/policy.json',
    'contracts/domain/v1/operations.json', 'contracts/domain/v1/common.schema.json', 'contracts/realtime/v1/world.schema.json',
    'frontend/src/shared/api/http.ts', 'frontend/src/shared/api/errors.ts', 'frontend/src/shared/api/domain.ts',
    'frontend/src/shared/api/domain-operations.generated.ts', 'frontend/src/shared/query/privacy.ts', 'frontend/src/shared/query/keys.ts',
    'frontend/src/features/auth/model/session.ts', 'frontend/src/features/social-sharing/model/controller.ts',
    'frontend/src/features/social-sharing/model/contract.ts', 'frontend/src/features/social-sharing/model/runtime.ts',
    'frontend/src/entities/intro/model/contract.ts', 'frontend/src/features/event-feed/contract.ts',
    'frontend/src/features/party-discovery/model/contract.ts', 'frontend/src/features/gift-discovery/model/contract.ts',
    'frontend/src/features/safety/contract.ts',
    'frontend/src/features/meet-beacon/model/contract.ts',
    ...['approved-social-request', 'public-intro', 'public-post', 'party-full', 'giveaway-management', 'private-beacon'].map(name => `contracts/fixtures/domain/${name}.json`),
    ...['fixtures.ts', 'oracle.ts', 'browser.ts', 'runtime.mjs', 'run.mjs'].map(name => `frontend/e2e/privacy/${name}`),
  ]
  for (const module of server.environments.client.moduleGraph.idToModuleMap.values()) {
    if (module.file?.startsWith(root + '/') && /\.(?:ts|tsx|json)$/.test(module.file) && !module.file.includes('/node_modules/')) files.push(module.file.slice(root.length + 1))
  }
  const sourcePins = Object.fromEntries(await Promise.all([...new Set(files)].sort().map(async file => [file, createHash('sha256').update(await readFile(resolve(root, file))).digest('hex')])))
  Object.assign(report, { browser: browser.version(), node: process.version, candidate: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
    command: 'NODE frontend/e2e/privacy/run.mjs', exitCode: 0, sourcePins, actualApiNetworkCalls: 0, pageErrors: errors })
  await writeFile(resolve(output, 'fixture-report.json'), JSON.stringify(report, null, 2) + '\n')
  console.log(JSON.stringify({ result: report.result, passed: 28, unsupportedWS: 7, projections: report.projections.length, pageErrors: errors.length }))
} finally {
  await browser?.close()
  await server.close()
  await rm(resolve(here, '.cache'), { recursive: true, force: true })
}
