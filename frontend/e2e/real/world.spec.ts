import { readFile, writeFile } from 'node:fs/promises'
import { test, expect, type Page, type BrowserContext } from '@playwright/test'
import { loadConfig, Prerequisite, sha256, type GateConfig, type Account } from './world-gate/config'
import { WorldObserver } from './world-gate/observer'
import { loadDecoder } from './world-gate/decoder'

type Status = 'PASS' | 'FAIL' | 'NOT_RUN'
type Check = { status: Status; code?: string }
const required = ['configuration', 'services', 'independentAccounts', 'approvedLiveWorld', 'bidirectionalMovement', 'panels', 'theme', 'customization', 'reconnect', 'singleGame', 'actualArtVisual', 'teardown'] as const

test('IN01 two independently authenticated users in the real shipped world', async ({ playwright }, info) => {
  const checks: Record<string, Check> = Object.fromEntries(required.map(k => [k, { status: 'NOT_RUN', code: 'PREREQUISITES_NOT_SATISFIED' }]))
  const report = { kind: 'mcr-in01-world-gate-v1', status: 'NOT_RUN' as Status, checks,
    observedSockets: [] as { opens: number; peak: number; activeAfterClose: number; snapshots: number; invalidFrames: number; nonzeroInputs: number }[],
    harnessBaseline: '8bb878c979138d0f6b3a4b880edb394f41cf99be', sourceHashes: {} as Record<string, string>,
    declaredRelease: '', receiptHashes: {} as Record<string, string>,
    evidenceScope: 'Real configured deployment only; no service launch, network/art mocks, fabricated accounts, trace or credential export.',
    gameCount: 'NOT_RUN: no reviewed product lifecycle API at source pin 8bb878c',
    visualReview: 'NOT_RUN: screenshots alone do not constitute actual-art approval' }
  const contexts: BrowserContext[] = [], observers: WorldObserver[] = []
  let browser: Awaited<ReturnType<typeof playwright.chromium.launch>> | undefined, config: GateConfig | undefined
  let pages: [Page, Page] | undefined, live = false
  const run = async (name: string, work: () => Promise<void>) => {
    try { await work(); checks[name] = { status: 'PASS' } }
    catch (error) { checks[name] = error instanceof Prerequisite ? { status: 'NOT_RUN', code: error.code } : { status: 'FAIL', code: 'ASSERTION_OR_RUNTIME_FAILURE' } }
  }
  const needLive = () => { if (!live || !pages || !config) throw new Prerequisite('TWO_LIVE_AUTHORIZED_WORLDS_REQUIRED') }
  try {
    for (const path of ['./world.spec.ts', './world-gate/config.ts', './world-gate/observer.ts', './world-gate/decoder.ts', './world-gate/runner.ts', '../../../contracts/realtime/v1/world.schema.json', '../../../contracts/realtime/v1/policy.json']) report.sourceHashes[path] = sha256(await readFile(new URL(path, import.meta.url)))
    await run('configuration', async () => {
      config = await loadConfig(); report.declaredRelease = config.releaseRevision
      report.receiptHashes = { services: config.serviceReceipt.sha256, artReview: config.artReview.sha256 }
    })
    if (config) {
      await run('services', async () => {
        const decode = await loadDecoder()
        try { browser = await playwright.chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }) } catch { throw new Prerequisite('BROWSER_EXECUTABLE_UNAVAILABLE') }
        for (let i = 0; i < 2; i++) {
          const context = await browser.newContext({ viewport: { width: 390, height: 844 }, serviceWorkers: 'block' }); contexts.push(context)
          const page = await context.newPage(), observer = new WorldObserver(decode); observers.push(observer); observer.attach(page, config!.origin)
          const response = await page.goto(`${config!.origin}/login?returnTo=%2Fworld`, { timeout: 15000 }).catch(() => { throw new Prerequisite('PRODUCT_ORIGIN_UNAVAILABLE') })
          if (!response?.ok()) throw new Prerequisite('PRODUCT_ORIGIN_UNAVAILABLE')
        }
        pages = [contexts[0]!.pages()[0]!, contexts[1]!.pages()[0]!]
      })
      if (pages) {
        await run('independentAccounts', async () => {
          async function login(page: Page, account: Account) {
            const submit = page.getByRole('button', { name: 'Sign in with email', exact: true })
            try { await expect(submit).toBeEnabled({ timeout: 15000 }) } catch { throw new Prerequisite('REAL_EMAIL_SIGN_IN_UNAVAILABLE') }
            await page.getByLabel('Email', { exact: true }).fill(account.email); await page.getByLabel('Password', { exact: true }).fill(account.password)
            const response = page.waitForResponse(r => new URL(r.url()).pathname === '/api/auth/login' && r.request().method() === 'POST', { timeout: 15000 })
            await submit.click(); const result = await response
            if (!result.ok()) throw new Prerequisite('PROVISIONED_ACCOUNT_LOGIN_REJECTED')
            // Parse the actual login body transiently for identity; token fields are never retained or exported.
            const body: unknown = await result.json()
            expect((body as { userId?: string }).userId).toBe(account.userId)
          }
          await login(pages![0], config!.accounts[0]); await login(pages![1], config!.accounts[1])
          expect(contexts[0]).not.toBe(contexts[1])
        })
        await run('approvedLiveWorld', async () => {
          if (checks.independentAccounts!.status !== 'PASS') throw new Prerequisite('TWO_REAL_LOGINS_REQUIRED')
          for (let i = 0; i < 2; i++) {
            try { await expect(pages![i]!.locator('[data-world-host]')).toContainText('World connected.', { timeout: 20000 }) }
            catch { throw new Prerequisite('WORLD_SERVICE_CONFIG_OR_APPROVED_ART_UNAVAILABLE') }
            // Missing renderer has a distinct truthful message even if the map/socket connected.
            if (/artwork is unavailable|Loading avatar/.test(await pages![i]!.locator('[data-world-host]').innerText())) throw new Prerequisite('APPROVED_AVATAR_RENDERER_UNAVAILABLE')
            await expect(pages![i]!.locator('[data-world-host] canvas')).toHaveCount(1)
          }
          await expect.poll(() => observers.every((o, i) => o.baseline?.selfUserId === config!.accounts[i]!.userId && o.baseline.eventId === config!.eventId && config!.accounts.every(a => o.players.has(a.userId))), { timeout: 15000 }).toBe(true)
          expect(observers[0]!.baseline!.roomId).toBe(observers[1]!.baseline!.roomId)
          expect(observers.every(o => o.active === 1 && o.peak === 1 && o.invalidFrames === 0)).toBe(true)
          live = true
        })
      }
      await run('bidirectionalMovement', async () => {
        needLive()
        for (let actor = 0; actor < 2; actor++) {
          const id = config!.accounts[actor]!.userId, peer = observers[1 - actor]!, own = observers[actor]!
          const before = peer.players.get(id)!.position, inputCount = own.nonzeroInputs
          const stick = pages![actor]!.getByRole('button', { name: 'Movement joystick. Arrow keys or W A S D when focused', exact: true })
          await stick.focus()
          // Bounded cardinal attempts handle a real spawn adjacent to a wall without assuming map geometry.
          for (const key of ['ArrowRight', 'ArrowDown', 'ArrowLeft', 'ArrowUp']) {
            await pages![actor]!.keyboard.down(key)
            try { await pages![actor]!.waitForTimeout(250) } finally { await pages![actor]!.keyboard.up(key) }
            if (peer.players.get(id)?.position.x !== before.x || peer.players.get(id)?.position.y !== before.y) break
          }
          await expect.poll(() => own.nonzeroInputs > inputCount && !!peer.players.get(id) && (peer.players.get(id)!.position.x !== before.x || peer.players.get(id)!.position.y !== before.y), { timeout: 10000 }).toBe(true)
          await expect.poll(() => { const a = own.players.get(id)?.position, b = peer.players.get(id)?.position; return !!a && !!b && a.x === b.x && a.y === b.y }).toBe(true)
        }
      })
      await run('panels', async () => {
        needLive()
        for (const [i, page] of pages!.entries()) {
          const host = await page.locator('[data-world-host]').elementHandle(), canvas = await page.locator('[data-world-host] canvas').elementHandle(), opens = observers[i]!.opens
          for (const name of ['Feed', 'Meet', 'Gifts', 'Wall', 'Me', 'World']) {
            await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name, exact: true }).click()
            expect(await page.locator('[data-world-host]').evaluate((el, before) => el === before, host)).toBe(true)
            expect(await page.locator('[data-world-host] canvas').evaluate((el, before) => el === before, canvas)).toBe(true)
            if (name === 'Feed') {
              await page.getByRole('heading', { level: 1, name: 'Feed', exact: true }).focus()
              const inputs = observers[i]!.nonzeroInputs
              await page.keyboard.down('ArrowRight')
              try { await page.waitForTimeout(250) } finally { await page.keyboard.up('ArrowRight') }
              expect(observers[i]!.nonzeroInputs).toBe(inputs)
            }
          }
          expect(observers[i]!.opens).toBe(opens); expect(observers[i]!.peak).toBe(1)
        }
      })
      await run('theme', async () => {
        needLive()
        const page = pages![0], baseline = observers[0]!.baseline!, avatar = observers[0]!.players.get(config!.accounts[0].userId)!.avatar
        await page.getByRole('link', { name: /Revenge \/ Parade \/ Danger/ }).click()
        const choices = page.getByRole('radio', { name: /^(REVENGE|PARADE|DANGER)$/ })
        if (await choices.count() !== 3) throw new Prerequisite('THEME_SELECTOR_NOT_COMPOSED')
        const selected = await page.locator('[data-mcr-theme]').getAttribute('data-mcr-theme')
        const next = selected === 'REVENGE' ? 'PARADE' : 'REVENGE'
        const opens = observers[0]!.opens, position = observers[0]!.players.get(config!.accounts[0].userId)!.position
        await page.getByRole('radio', { name: next, exact: true }).check()
        await expect(page.locator('[data-mcr-theme]')).toHaveAttribute('data-mcr-theme', next)
        expect(observers[0]!.baseline!.roomId).toBe(baseline.roomId); expect(observers[0]!.baseline!.mapId).toBe(baseline.mapId)
        expect(observers[0]!.players.get(config!.accounts[0].userId)!.avatar).toEqual(avatar)
        expect(observers[0]!.players.get(config!.accounts[0].userId)!.position).toEqual(position); expect(observers[0]!.opens).toBe(opens)
        // DOM selection does not prove persisted preference or unchanged rendered geometry.
        throw new Prerequisite('THEME_PERSISTENCE_AND_RUNTIME_SKIN_OBSERVATION_REQUIRED')
      })
      await run('customization', async () => {
        needLive()
        const page = pages![0]
        await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Me', exact: true }).click()
        await page.getByRole('link', { name: 'Dress up', exact: true }).click()
        if (await page.getByRole('heading', { name: 'Dress up is not available yet', exact: true }).count()) throw new Prerequisite('AVATAR_EDITOR_NOT_COMPOSED')
        // No editor is composed at the assigned pin. Do not invent save/preset controls for a future implementation.
        throw new Prerequisite('AVATAR_SAVE_INTERACTION_CONTRACT_REQUIRES_SOURCE_REVIEW')
      })
      await run('reconnect', async () => {
        needLive()
        const page = pages![0], observer = observers[0]!, snapshots = observer.snapshots, stream = observer.baseline!.streamId
        await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'World', exact: true }).click()
        await contexts[0]!.setOffline(true)
        try { await expect.poll(() => observer.active, { timeout: 15000 }).toBe(0) }
        finally { await contexts[0]!.setOffline(false) }
        const reconnect = page.getByRole('button', { name: 'Reconnect world', exact: true })
        await expect(reconnect).toBeVisible(); await reconnect.click()
        await expect.poll(() => observer.snapshots > snapshots && observer.baseline?.streamId !== stream && config!.accounts.every(a => observer.players.has(a.userId)), { timeout: 20000 }).toBe(true)
        expect(observer.active).toBe(1); expect(observer.peak).toBe(1); expect(observer.invalidFrames).toBe(0)
      })
      await run('singleGame', async () => { needLive(); throw new Prerequisite('READ_ONLY_GAME_LIFECYCLE_SEAM_REQUIRED') })
      await run('actualArtVisual', async () => {
        needLive()
        for (const [i, page] of pages!.entries()) {
          await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'World', exact: true }).click()
          await info.attach(`world-art-context-${i + 1}`, { body: await page.locator('[data-world-host]').screenshot(), contentType: 'image/png' })
        }
        throw new Prerequisite('CURRENT_ART_SCREENSHOTS_REQUIRE_REVIEW')
      })
    }
  } finally {
    await run('teardown', async () => {
      if (!contexts.length) throw new Prerequisite('NO_BROWSER_CONTEXTS_ALLOCATED')
      for (const context of contexts) await context.close()
      await expect.poll(() => observers.every(o => o.active === 0)).toBe(true)
    })
    await browser?.close()
    report.observedSockets = observers.map(o => ({ opens: o.opens, peak: o.peak, activeAfterClose: o.active, snapshots: o.snapshots, invalidFrames: o.invalidFrames, nonzeroInputs: o.nonzeroInputs }))
    observers.forEach(o => o.clear())
    report.status = Object.values(checks).some(c => c.status === 'FAIL') ? 'FAIL' : Object.values(checks).every(c => c.status === 'PASS') ? 'PASS' : 'NOT_RUN'
    const reportPath = info.outputPath('in01-result.json')
    await writeFile(reportPath, JSON.stringify(report, null, 2) + '\n', { mode: 0o600 })
    await info.attach('in01-result', { path: reportPath, contentType: 'application/json' })
  }
  // Missing prerequisites are an explicit nonzero gate, never a skipped or passing IN01 test.
  if (report.status !== 'PASS') throw new Error(`IN01 ${report.status}: ${Object.entries(checks).filter(([, c]) => c.status !== 'PASS').map(([key, c]) => `${key}=${c.code}`).join('; ')}`)
})
