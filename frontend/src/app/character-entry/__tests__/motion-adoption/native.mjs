import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { mkdir, mkdtemp, readFile, writeFile, rm } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { aliases } from '../../../../world/avatar/__tests__/motion-adoption/vitest.config.mjs'
import { producerInputs, readPinnedProducer } from '../../../../world/avatar/__tests__/motion-adoption/producer-inputs.ts'

const dependencies = process.env.MCR_FRONTEND_DEPENDENCIES
if (!dependencies) throw Error('Set MCR_FRONTEND_DEPENDENCIES')
const require = createRequire(resolve(dependencies, 'package.json'))
const { createServer } = require('vite'), { chromium, expect } = require('@playwright/test')
const root = resolve(import.meta.dirname, '../../../../..'), output = resolve(root, '../docs/mcr/evidence/avatar-motion-adoption')
await mkdir(output, { recursive: true }); const cache = await mkdtemp('/private/tmp/mcr-motion-native-')
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
const producerCommit = producerInputs.commit
const producer = readPinnedProducer
const candidateBytes = producer('backend/releases/selected-character-motion-v2/manifest.json')
assert.equal(candidateBytes.length, 95128); assert.equal(hash(candidateBytes), '9a555f726a9e421f14ed876117be87bb1bb8fa128ac401ab38f8927760776fd6')
const presets = Array.from({ length: 6 }, (_, i) => JSON.parse(new TextDecoder().decode(producer(`contracts/fixtures/assets/motion-v2/valid/preset-0${i + 1}.json`))))
let saved, mode = 'ok'
const reset = () => { saved = { revision: 1, config: presets[0] }; mode = 'ok' }
reset()
const report = { result: 'FAIL', qualification: 'MOCK_READY: new editor/controller/runtime and actual incumbent Phaser scene under explicit TEST_ONLY local authority and deterministic HTTP fixture. Not application Root/public C05/production admission, live socket/service, physical device or multiuser acceptance.',
  parent: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), cases: [], errors: [], artifacts: {} }
report.producer = { commit: producerCommit, manifestSha256: hash(candidateBytes), productionEligible: false, wireContract: '2.0.0', qualification: 'TEST_ONLY' }
const reply = (res, status, value) => { res.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }); res.end(JSON.stringify(value)) }
const server = await createServer({ configFile: false, root, cacheDir: cache, logLevel: 'error',
  resolve: { alias: Object.entries(aliases).sort(([a], [b]) => b.length - a.length).map(([find, replacement]) => ({ find, replacement })) },
  server: { host: '127.0.0.1', port: 0, fs: { allow: [root, resolve(root, '..'), dependencies] } },
  plugins: [{ name: 'explicit-test-only-motion-http', configureServer(server) {
    server.middlewares.use(async (req, res, next) => {
      if (req.url === '/__motion/candidate') {
        res.writeHead(200, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }); res.end(candidateBytes); return
      }
      if (req.url === '/tools/wardrobe-review.mjs') {
        res.writeHead(200, { 'Content-Type': 'text/javascript', 'Cache-Control': 'no-store' })
        res.end(await readFile(resolve(root, 'src/entities/avatar/derived-assets/motion-candidates-v1/wardrobe-review.mjs'))); return
      }
      if (req.url?.startsWith('/__motion/mode?')) {
        mode = new URL(req.url, 'http://fixture.test').searchParams.get('value')
        if (mode === 'rebase') { saved = { revision: saved.revision + 1, config: presets[5] }; mode = 'ok' }
        return reply(res, 200, {})
      }
      if (req.url !== '/__motion/me') return next()
      if (req.method === 'GET') return reply(res, 200, saved)
      if (req.method !== 'PUT') return reply(res, 405, {})
      const chunks = []; for await (const part of req) chunks.push(part)
      try {
        const body = JSON.parse(Buffer.concat(chunks).toString())
        assert.deepEqual(Object.keys(body).sort(), ['config', 'expected_revision'])
        const { decodeMotionV2Config } = await server.ssrLoadModule('/src/entities/avatar/runtime/motion-adoption/v2.ts')
        const c = decodeMotionV2Config(body.config)
        if (mode === 'denied') return reply(res, 403, { code: 'DENIED', message: 'Denied', details: { reason: 'PERMISSION_DENIED' } })
        if (mode === 'conflict' || body.expected_revision !== saved.revision) return reply(res, 409, { code: 'CONFLICT', message: 'Changed', details: { reason: 'VERSION_CONFLICT', current_revision: saved.revision } })
        saved = { revision: saved.revision + 1, config: c }
        if (mode === 'uncertain') return reply(res, 503, { code: 'UNAVAILABLE', message: 'Unknown result', details: { reason: 'DEPENDENCY_UNAVAILABLE' } })
        return reply(res, 200, saved)
      } catch { return reply(res, 400, { code: 'INVALID', message: 'Invalid test-only selection', details: { reason: 'INVALID_INPUT' } }) }
    })
  } }],
})
let browser, page, stage = 'start'
await server.listen(); const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const state = () => page.evaluate(() => window.motionJourney?.state())
const invoke = (method, ...args) => page.evaluate(async ({ method, args }) => window.motionJourney[method](...args), { method, args })
const modeTo = value => page.request.get(`${origin}/__motion/mode?value=${value}`)
const noOverflow = () => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)
try {
  browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true }); report.browser = browser.version()
  for (const [width, height] of [[360, 800], [390, 844], [430, 932]]) {
    reset(); stage = `${width}/startup`; page = await browser.newPage({ viewport: { width, height }, reducedMotion: 'reduce' })
    page.setDefaultTimeout(12000); page.on('pageerror', error => report.errors.push(error.message))
    await page.goto(`${origin}/src/app/character-entry/__tests__/motion-adoption/fixture.html`)
    await expect.poll(async () => (await state())?.scene.sprites).toBe(2)
    const initial = await state(); assert.equal(initial.mounts, 1); assert.equal(initial.gameCanvases, 1); assert.equal(initial.requests.length, 2)
    const artProof = await invoke('artProof'); assert.equal(artProof.compared, 120); assert(artProof.distinct.every(n => n === 5))
    if (width === 360) {
      stage = 'independent wardrobe reference'
      const cases = []
      for (let body = 0; body < 8; body++) for (let outfit = 0; outfit < 6; outfit++) for (let direction = 0; direction < 4; direction++) for (let phase = 0; phase < 5; phase++) cases.push({ body, hair: outfit, eyes: body % 4, outfit, accessory: null, direction, phase })
      for (let hair = 0; hair < 6; hair++) for (const accessory of [null, 0, 1, 2, 3]) for (let direction = 0; direction < 4; direction++) cases.push({ body: 7, hair, eyes: 0, outfit: 5, accessory, direction, phase: 1 })
      for (let eyes = 0; eyes < 4; eyes++) for (let hair = 0; hair < 6; hair++) for (let direction = 0; direction < 4; direction++) cases.push({ body: 0, hair, eyes, outfit: hair, accessory: null, direction, phase: 0 })
      const reference = await browser.newPage()
      await reference.goto(`${origin}/src/entities/avatar/derived-assets/motion-candidates-v1/wardrobe-review.html`)
      await expect(reference.locator('#state')).toContainText('idle')
      const expected = await reference.evaluate(async cases => {
        const canvas = document.createElement('canvas'); canvas.width = canvas.height = 64; const ctx = canvas.getContext('2d'), hashes = []
        const sha = async bytes => 'sha256:' + [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(n => n.toString(16).padStart(2, '0')).join('')
        for (const entry of cases) { window.__WARDROBE__.compose(ctx, entry, entry.phase); hashes.push(await sha(ctx.getImageData(0, 0, 64, 64).data.buffer)) }
        return { cases: hashes.length, aggregate: await sha(new TextEncoder().encode(hashes.join('\n'))) }
      }, cases)
      await reference.close(); const actual = await invoke('wardrobeProof', cases); assert.deepEqual(actual, expected)
      report.wardrobeReference = { ...actual, reference: 'Unchanged previously qualified wardrobe-review.mjs Canvas fixture; 1176 pairwise RGBA comparisons.' }
    }
    stage = `${width}/controls`; await page.getByRole('button', { name: 'CUSTOMIZE', exact: true }).click()
    await expect(page.getByRole('img', { name: 'Draft outfit preview', exact: true })).toBeVisible()
    // Selecting an actual category from preset mode explicitly starts a wardrobe draft.
    await page.getByRole('button', { name: width === 360 ? 'Hair' : 'Dress up', exact: true }).click()
    await expect.poll(async () => (await state()).controller.draft?.representation).toBe('motion_layered')
    assert.equal((await state()).saved.config.representation, 'motion_preset')
    await page.getByRole('button', { name: 'Skin / body', exact: true }).click(); await page.getByLabel('Choose skin / body', { exact: true }).selectOption('body-7')
    await page.getByRole('button', { name: 'Hair', exact: true }).click(); await page.getByLabel('Choose hair', { exact: true }).selectOption('hair-5')
    await page.getByRole('button', { name: 'Eyes', exact: true }).click(); await page.getByLabel('Choose eyes', { exact: true }).selectOption('eyes-3')
    await page.getByRole('button', { name: 'Outfit', exact: true }).click(); await page.getByLabel('Choose outfit', { exact: true }).selectOption('outfit-5')
    await page.getByRole('button', { name: 'Accessory', exact: true }).click(); await page.getByLabel('Choose accessory', { exact: true }).selectOption('accessory-2')
    const draft = (await state()).controller.draft; assert.equal(draft.contract_version, '2.0.0'); assert.equal(draft.representation, 'motion_layered')
    assert.deepEqual(draft.item_ids, { body: 'motion_v2_body_7', eyes: 'motion_v2_eyes_3', hair: 'motion_v2_hair_5', outfit: 'motion_v2_outfit_5', accessory: 'motion_v2_accessory_2' })
    const directionFrames = []
    await page.getByRole('button', { name: 'Preview walk', exact: true }).click()
    for (const direction of ['down', 'left', 'right', 'up']) {
      await page.getByLabel('View', { exact: true }).selectOption(direction)
      const values = new Set(); for (let i = 0; i < 12; i++) { values.add(await page.getByRole('img', { name: 'Draft outfit preview', exact: true }).getAttribute('data-frame')); await page.waitForTimeout(65) }
      assert.equal(values.size, 4); directionFrames.push({ direction, frames: [...values] })
    }
    await page.getByRole('button', { name: 'Stop walking', exact: true }).click()
    await expect(page.getByRole('img', { name: 'Draft outfit preview', exact: true })).toHaveAttribute('data-frame', '15')
    assert(await noOverflow()); assert.equal((await state()).requests.length, 2)
    const preview = await page.getByRole('img', { name: 'Draft outfit preview', exact: true }).evaluate(canvas => canvas.toDataURL())
    for (const index of [0, 1, 2]) {
      await invoke('theme', index)
      assert.equal(await page.getByRole('img', { name: 'Draft outfit preview', exact: true }).evaluate(canvas => canvas.toDataURL()), preview)
      assert(await noOverflow()); const file = `customize-${width}-${['revenge', 'parade', 'danger'][index]}.jpg`
      await page.screenshot({ path: resolve(output, file), type: 'jpeg', quality: 86, fullPage: true }); report.artifacts[file] = hash(await readFile(resolve(output, file)))
    }
    if (width === 390) {
      await page.getByRole('button', { name: 'Hair', exact: true }).click()
      await expect(page.getByRole('button', { name: 'Choose hair 6', exact: true })).toHaveAttribute('aria-pressed', 'true')
      await page.getByRole('button', { name: 'Dress up', exact: true }).click()
      assert.deepEqual((await state()).controller.draft, draft)
      await page.getByLabel('View', { exact: true }).selectOption('down')
      await expect(page.getByRole('img', { name: 'Draft outfit preview', exact: true })).toHaveAttribute('data-frame', '0')
      const rail = await page.getByRole('group', { name: 'Clothing categories', exact: true }).boundingBox()
      const previewBox = await page.getByRole('img', { name: 'Draft outfit preview', exact: true }).boundingBox()
      const controls = await page.getByRole('group', { name: 'Clothing categories', exact: true }).getByRole('button').evaluateAll(buttons => buttons.map(button => {
        const r = button.getBoundingClientRect(); return { label: button.getAttribute('aria-label'), x: r.x, y: r.y, width: r.width, height: r.height }
      }))
      assert.equal(controls.length, 5); assert(controls.every((button, i) => button.width >= 44 && button.height >= 44 && (!i || button.y > controls[i - 1].y)))
      assert(previewBox.x + previewBox.width <= rail.x)
      const panel = page.getByRole('region', { name: 'Avatar editor', exact: true }), panelBox = await panel.boundingBox()
      assert(panelBox.height <= 698)
      const file = 'customize-reference-fit-390.jpg'
      await panel.screenshot({ path: resolve(output, file), type: 'jpeg', quality: 90 }); report.artifacts[file] = hash(await readFile(resolve(output, file)))
      report.referenceFit = { qualification: 'Actual candidate component at 390px viewport, not production Root or legacy-character substitution', viewportWidth: width, panel: panelBox,
        rightCategoryRail: controls, preview: previewBox, selectedSourceConfig: draft, artifact: file }
      await page.getByRole('button', { name: 'Accessory', exact: true }).click()
      await page.getByLabel('View', { exact: true }).selectOption('up')
      await expect(page.getByRole('img', { name: 'Draft outfit preview', exact: true })).toHaveAttribute('data-frame', '15')
    }
    stage = `${width}/save`; await page.getByRole('button', { name: 'Save outfit', exact: true }).click(); await expect(page.getByText('Outfit saved.', { exact: true })).toBeVisible()
    assert.deepEqual((await state()).saved.config, draft)
    const file = `customize-${width}.jpg`; await page.screenshot({ path: resolve(output, file), type: 'jpeg', quality: 86, fullPage: true }); report.artifacts[file] = hash(await readFile(resolve(output, file)))
    stage = `${width}/profile`; await page.getByRole('button', { name: 'PROFILE', exact: true }).click()
    await expect(page.getByRole('img', { name: 'Saved profile avatar', exact: true })).toBeVisible()
    const pixels = await invoke('pixels'); assert.equal(pixels.profile, pixels.sharedComposition)
    const worldFrames = []
    for (const [row, direction] of ['down', 'left', 'right', 'up'].entries()) {
      await invoke('move', direction, true)
      await expect.poll(async () => (await state()).distinctSpriteFrames.toSorted((a, b) => a - b)).toEqual([1, 2, 3, 4].map(phase => row * 5 + phase))
      const frames = (await state()).distinctSpriteFrames
      assert.deepEqual(frames.sort((a, b) => a - b), [1, 2, 3, 4].map(phase => row * 5 + phase)); worldFrames.push({ direction, frames })
      await invoke('move', direction, false); await expect.poll(async () => (await state()).actualSpriteFrames.at(-1)?.frame).toBe(row * 5)
    }
    const geometry = (await state()).geometry
    for (const index of [0, 1, 2]) { await invoke('theme', index); assert.equal((await state()).geometry, geometry); assert.equal((await state()).requests.length, 2); assert.equal((await state()).mounts, 1) }
    stage = `${width}/uncertain`; await page.getByRole('button', { name: 'CUSTOMIZE', exact: true }).click(); await expect(page.getByRole('img', { name: 'Draft outfit preview', exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Presets', exact: true }).click(); await page.getByRole('button', { name: 'Choose Preset 3', exact: true }).click()
    await modeTo('uncertain'); await page.getByRole('button', { name: 'Save outfit', exact: true }).click()
    await expect(page.getByText('The save result is unknown. Check the saved outfit before another save.', { exact: true })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Save outfit', exact: true })).toBeDisabled()
    await modeTo('ok'); await page.getByRole('button', { name: 'Check latest saved outfit', exact: true }).click()
    await page.getByRole('button', { name: 'Use saved outfit', exact: true }).click()
    assert.equal((await state()).controller.needsReconciliation, false)
    stage = `${width}/privacy`; await invoke('privacy'); const revoked = await state()
    assert.equal(revoked.scene.sprites, 0); assert.equal(revoked.scene.textures, 0); assert.equal(revoked.owner.leases, 0); assert.equal(revoked.controller.draft, null)
    const cleanup = await invoke('dispose'); assert.equal(cleanup.canvases, 0); assert.equal(cleanup.mounts, 1); assert.equal(cleanup.destroys, 1)
    assert.deepEqual(cleanup.scene, { textures: 0, sprites: 0, listeners: 0 }); assert.equal(cleanup.owner.decodedAtlases, 0)
    report.cases.push({ width, height, initial: { mounts: initial.mounts, gameCanvases: initial.gameCanvases, avatarRequests: initial.requests.length }, artProof, directionFrames, worldFrames, draft, pixels, cleanup })
    await page.close()
  }
  assert.deepEqual(report.errors, []); report.result = 'PASS'
} catch (error) { report.failure = { stage, error: String(error) }; if (page) { await page.screenshot({ path: resolve(output, 'failure.jpg'), type: 'jpeg', quality: 80, fullPage: true }).catch(() => {}); report.failure.dom = await page.locator('body').innerText().catch(() => 'unavailable') }; throw error }
finally { await writeFile(resolve(output, 'native.json'), JSON.stringify(report, null, 2) + '\n'); await browser?.close(); await server.close(); await rm(cache, { recursive: true, force: true }) }
console.log(JSON.stringify({ result: report.result, cases: report.cases.length, qualification: report.qualification }))
