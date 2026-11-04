import assert from 'node:assert/strict'
import { randomUUID } from 'node:crypto'
import { statfsSync } from 'node:fs'
import { isAbsolute } from 'node:path'
import { lstat, readFile, writeFile } from 'node:fs/promises'
import { test, expect } from '@playwright/test'
import type { Browser, BrowserContext, Request } from '@playwright/test'
import { Actor, NotRun, hash, loadConfig, need, verifyProof } from '../real/mural-helpers.ts'
import type { Admin, GateConfig, Snapshot } from '../real/mural-helpers.ts'
import { loadDecoder } from '../real/world-gate/decoder'
import { WorldObserver } from '../real/world-gate/observer'
import { loadWallContracts, bindAction, boundedSeed, confirmLabels, connectedRegion, expectedEffect, mutations, originalKey, preview, reason, recover, rendered, requireInputs, tools } from './helpers.ts'
import type { Command, OwnAction } from './helpers.ts'
import { loadRuntimeAdapter, qualifiedBinding } from './runtime.ts'
import type { RuntimeAdapter } from './runtime.ts'
import { verifyWorldTexture } from './world-texture.ts'
import { captureReconnectBefore, reconnectWorldOnce } from './reconnect.ts'

const START_FLOOR_BYTES = 11 * 1024 ** 3, LIVE_FLOOR_BYTES = 8 * 1024 ** 3, RESIDUAL_BYTES = 9 * 1024 ** 3
const stages = ['inputs', 'deployment', 'independentActors', 'baseline', 'pencil', 'sharedCooldown', 'opaqueErase', 'boundedFill', 'eligibleUndo', 'reloadBudget', 'remoteInvalidatesUndo', 'lostActionRecovery', 'lostPencilRecovery', 'privateResults', 'unauthorizedModeration', 'pause', 'regionLock', 'restore', 'obsoleteDelta', 'reconnect', 'restartEpoch', 'finalFreeze', 'teardown'] as const
test('IN04 Wall B real canonical clients, tools, recovery, moderation and freeze', async ({ playwright }, info) => {
  const checks = Object.fromEntries(stages.map(s => [s, { status: 'NOT_RUN', code: 'PREREQUISITES_REQUIRED' }])) as Record<typeof stages[number], { status: 'PASS' | 'FAIL' | 'NOT_RUN'; code?: string }>
  const report = { kind: 'IN04_WALL_B_REAL_ONLY', status: 'NOT_RUN', sourceBase: '2ff0aab865a06a5664d2096ba572c766ac0fba85', checks,
    candidate: '', eventId: '', lastRevision: 0, finalState: 'NOT_RUN', sourceHashes: {} as Record<string, string>,
    requests: [] as { actor: string; operation: string; key: string; bodyHash: string }[],
    convergence: [] as { revision: number; snapshotId: string; pixelsHash: string; state: string }[],
    actions: [] as { actor: string; key: string; operation: string; actionId: string; revision: number; changedCells: number }[],
    limitations: 'No mock mode, service provisioning, fabricated authority, visual matrix, load or performance qualification. On uncertainty retain exact keys and have the actual operator restore; never retry writes automatically.' }
  let config: GateConfig, port: RuntimeAdapter, browser: Browser | undefined, deadline = 0, current: typeof stages[number] = 'inputs'
  let timer: ReturnType<typeof setTimeout> | undefined
  const contexts: BrowserContext[] = [], actors: Actor[] = [], observers: WorldObserver[] = []
  const failures: string[] = [], pageErrors: string[] = [], retained = new Map<string, { command: Command; actor: number }>()
  let baseline: Snapshot
  const step = async (name: typeof stages[number], run: () => Promise<void>) => {
    current = name
    if (name !== 'inputs' && name !== 'teardown') {
      need(Date.now() < deadline, 'ACCEPTED_WORK_DEADLINE_EXPIRED')
      const disk = statfsSync(new URL('.', import.meta.url))
      const liveFloor = Number(process.env.MCR_IN04_B_LIVE_FLOOR_BYTES ?? LIVE_FLOOR_BYTES)
      need(liveFloor >= LIVE_FLOOR_BYTES && disk.bavail * disk.bsize >= liveFloor, 'ACCEPTED_LIVE_DISK_FLOOR_REQUIRED')
    }
    await run(); checks[name] = { status: 'PASS' }
  }
  const member = (n: number) => actors[n]!, page = (n: number) => member(n).page, operator = () => member(2)
  const fresh = () => member(0).snapshot()
  const alive = async () => {
    need(Date.now() < deadline, 'ACCEPTED_WORK_DEADLINE_EXPIRED')
    verifyProof(config, await port.inspect(config))
  }
  const converge = async (expected?: number[]) => {
    const s = await fresh()
    if (expected) assert.deepEqual(s.pixels, expected)
    for (let n = 0; n < 2; n++) {
      await expect.poll(async () => {
        try {
          const binding = await qualifiedBinding(port, config)
          assert.equal(observers[n]!.baseline?.mapId, binding.configuration.map.id)
          assert.equal(observers[n]!.baseline?.mapRevision, binding.configuration.map.revision)
          await rendered(page(n), s); await verifyWorldTexture(page(n), s, binding); return true
        } catch { return false }
      }, { timeout: Math.min(15000, Math.max(1, deadline - Date.now())) }).toBe(true)
      const peer = await member(n).snapshot()
      assert.equal(peer.revision, s.revision); assert.equal(peer.snapshot_id, s.snapshot_id)
      assert.equal(peer.state, s.state); assert.deepEqual(peer.pixels, s.pixels)
      assert.equal(await page(n).locator('[data-world-host] canvas').count(), 1)
      assert.equal(observers[n]!.peak, 1); assert.equal(observers[n]!.active, 1); assert.equal(observers[n]!.invalidFrames, 0)
    }
    report.lastRevision = s.revision
    report.convergence.push({ revision: s.revision, snapshotId: s.snapshot_id, pixelsHash: hash(JSON.stringify(s.pixels)), state: s.state })
    return s
  }
  const ready = async (n: number) => {
    await alive()
    const s = await member(n).snapshot(), wait = s.next_allowed_at ? Date.parse(s.next_allowed_at) - Date.parse(s.server_time) : 0
    need(wait <= config.maxCooldownMs && Date.now() + Math.max(0, wait) + 100 < deadline, 'BOUNDED_COOLDOWN_RESOURCE_WINDOW_REQUIRED')
    if (wait > 0) await page(n).waitForTimeout(wait + 100)
    return member(n).snapshot()
  }
  const denied = async (n: number, command: Command, why: string) => {
    const before = await fresh(), r = await member(n).call(command.operation_id, command.body, {}, {}, originalKey(command.key))
    assert([403, 409, 429].includes(r.status)); assert.equal(reason(r.value), why)
    const after = await fresh(); assert.equal(after.revision, before.revision); assert.deepEqual(after.pixels, before.pixels)
  }
  const paint = (s: Snapshot, x: number, y: number): Command => ({ operation_id: 'placeConcertPixel', key: randomUUID(), body: { expected_revision: s.revision, x, y, color_index: (s.pixels[y * 128 + x]! + 1) % 16 } })
  const uiAction = async (n: number, input: Command, before: Snapshot, undoBefore?: Snapshot, lose = false, both = true): Promise<OwnAction> => {
    const command = { ...input }, p = page(n), actor = member(n), path = actor.contracts.path(command.operation_id, config.eventId)
    const priorBudget = await tools(actor, before.revision)
    let actual: unknown, posts = 0
    const onRequest = (r: Request) => { if (new URL(r.url()).pathname === path && r.method() === 'POST') posts++ }
    p.on('request', onRequest)
    try {
      if (lose) await p.route(config.origin + path, async route => {
        command.key = route.request().headers()['idempotency-key']!
        assert.deepEqual(route.request().postDataJSON(), command.body)
        // Forward once to the genuine server, then discard only its actual response.
        const response = await route.fetch({ maxRetries: 0, timeout: 15000 })
        assert.equal(response.status(), 200); actual = await response.json()
        retained.set(command.key, { command, actor: n })
        await route.abort('failed')
      }, { times: 1 })
      await preview(p, command); assert.equal(posts, 0)
      const response = lose ? null : p.waitForResponse(r => new URL(r.url()).pathname === path && r.request().method() === 'POST')
      await p.getByRole('button', { name: confirmLabels[command.operation_id], exact: true }).click()
      if (response) {
        const r = await response; assert.equal(r.status(), 200)
        command.key = r.request().headers()['idempotency-key']!
        assert.deepEqual(r.request().postDataJSON(), command.body)
        actual = await r.json()
        retained.set(command.key, { command, actor: n })
      } else {
        await expect(p.getByRole('status', { name: 'Wall command status', exact: true })).toContainText(/unknown|pending/i)
        await p.reload() // Canonical cookie/session restore; no token/storage injection.
        await p.getByRole('region', { name: 'Wall editor', exact: true }).waitFor()
        const check = p.getByRole('button', { name: 'Check original command', exact: true })
        if (await check.isVisible()) await check.click()
      }
      assert.equal(posts, 1)
      const recovered = await recover(actor, command)
      assert.equal(recovered.state, 'SUCCEEDED')
      if (recovered.state !== 'SUCCEEDED') throw Error('EXACT_ORIGINAL_RESULT_REQUIRED')
      const action = recovered.action
      if (command.operation_id !== 'placeConcertPixel') assert.deepEqual(action, bindAction(actor.contracts, actual, command, actor.account.userId, config.eventId, recovered.receipt))
      else assert.equal((actual as { revision: number }).revision, action.revision)
      const expected = expectedEffect(before, command, undoBefore), after = both ? await converge(expected) : await member(n).snapshot()
      assert.deepEqual(after.pixels, expected)
      assert.equal(after.revision, before.revision + 1)
      const changed = expected.filter((color, i) => color !== before.pixels[i]).length
      assert.equal(changed, action.changed_cells)
      const budget = await tools(actor, after.revision)
      assert.equal(budget.remaining_cells, priorBudget.remaining_cells - changed)
      assert.equal(budget.next_allowed_at, action.next_allowed_at)
      if (command.operation_id !== 'placeConcertPixel') {
        const delta = await member(1 - n).delta(before.revision)
        assert.equal(delta.reset_required, true); assert.deepEqual(delta.operations, [])
      }
      report.actions.push({ actor: actor.account.userId, key: command.key, operation: command.operation_id, actionId: action.action_id, revision: action.revision, changedCells: action.changed_cells })
      return action
    } finally { p.off('request', onRequest); if (lose) await p.unroute(config.origin + path) }
  }
  const control = async (action: 'PAUSE' | 'RESUME' | 'FREEZE') => {
    await alive(); const before = await operator().admin()
    const r = await operator().call('controlConcertMural', { expected_revision: before.revision, action, reason: 'IN04 isolated Wall B journey' })
    assert.equal(r.status, 200)
    const state = operator().contracts.decode<Admin>('Domain_mural_MuralAdminState', r.value)
    assert.equal(state.event_id, config.eventId); assert.equal(state.revision, before.revision + 1)
    assert.equal(state.state, action === 'PAUSE' ? 'PAUSED' : action === 'FREEZE' ? 'FROZEN' : 'EDITABLE')
    return state
  }
  try {
    await step('inputs', async () => {
      const out = process.env.MCR_IN04_B_ARTIFACT_ROOT
      need(out && isAbsolute(out) && process.env.MCR_IN04_B_ALLOCATION, 'ACCEPTED_ALLOCATION_AND_PRIVATE_OUTPUT_REQUIRED')
      const startFloor = Number(process.env.MCR_IN04_B_START_FLOOR_BYTES ?? START_FLOOR_BYTES)
      const ownNew = Number(process.env.MCR_IN04_B_MAX_NEW_BYTES), allNew = Number(process.env.MCR_IN04_B_ALL_NEW_BYTES)
      const disk = statfsSync(new URL('.', import.meta.url)), free = disk.bavail * disk.bsize
      need(startFloor >= START_FLOOR_BYTES && free >= startFloor, 'FRESH_ACCEPTED_START_FLOOR_REQUIRED')
      need(Number.isSafeInteger(ownNew) && ownNew > 0 && ownNew <= 2 * 1024 ** 3
        && Number.isSafeInteger(allNew) && allNew >= ownNew && free - allNew >= RESIDUAL_BYTES, 'ALLOCATOR_ALL_NEW_AND_9GIB_RESIDUAL_REQUIRED')
      // This controller check cannot replace the separately allocated independent
      // guardian or change any accepted WHOLE/PID/RSS/deadline/cleanup ceiling.
      const st = await lstat(out)
      need(st.isDirectory() && !st.isSymbolicLink() && st.uid === process.getuid?.() && !(st.mode & 0o077), 'PRIVATE_OWNED_ARTIFACT_DIRECTORY_REQUIRED')
      deadline = Number(process.env.MCR_IN04_B_WORK_DEADLINE_MS)
      need(deadline > Date.now() && deadline <= Date.now() + 15 * 60000, 'CURRENT_BOUNDED_WORK_DEADLINE_REQUIRED')
      config = await loadConfig(); requireInputs(config); port = await loadRuntimeAdapter()
      await loadWallContracts(config)
      report.candidate = config.releaseRevision; report.eventId = config.eventId
      for (const p of ['./wall-b.spec.ts', './helpers.ts', './runtime.ts', './world-texture.ts', './reconnect.ts', '../real/mural-helpers.ts', '../../src/app/mural-entry/owner.ts', '../../src/app/world-entry/production.ts', '../../src/features/mural/editor/MuralEditor.tsx', '../../src/features/mural/sync/tools-http.ts', '../../../contracts/domain/v1/operations.json', '../../../contracts/domain/v1/mural.schema.json', '../../../contracts/generated/mcr-domain.openapi.json', '../../../contracts/realtime/v1/world.schema.json']) report.sourceHashes[p] = hash(await readFile(new URL(p, import.meta.url)))
    })
    await step('deployment', alive)
    await step('independentActors', async () => {
      browser = await playwright.chromium.launch({ executablePath: process.env.MCR_CHROME_EXECUTABLE ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true })
      timer = setTimeout(() => { void browser?.close().catch(() => {}) }, Math.max(1, deadline - Date.now()))
      const decode = await loadDecoder(), contracts = await loadWallContracts(config)
      for (const account of [...config.members, config.operator]) {
        const context = await browser.newContext({ viewport: { width: 390, height: 844 }, serviceWorkers: 'block' }); contexts.push(context)
        const p = await context.newPage(), actor = new Actor(p, context.request, account, config, contracts); actors.push(actor)
        p.on('pageerror', () => pageErrors.push('PRODUCT_PAGE_ERROR'))
        if (actors.length <= 2) {
          const observer = new WorldObserver(decode); observers.push(observer); observer.attach(p, config.origin)
          p.on('request', r => {
            const op = mutations.find(id => contracts.path(id, config.eventId) === new URL(r.url()).pathname)
            if (op && r.method() === 'POST') report.requests.push({ actor: account.userId, operation: op, key: r.headers()['idempotency-key'] ?? '', bodyHash: hash(r.postData() ?? '') })
          })
        }
        await actor.login()
      }
      assert.notEqual(contexts[0], contexts[1])
      for (let n = 0; n < 2; n++) await expect.poll(() => observers[n]!.baseline?.eventId === config.eventId && observers[n]!.baseline?.selfUserId === config.members[n]!.userId).toBe(true)
    })
    await step('baseline', async () => {
      baseline = await fresh(); const admin = await operator().admin()
      need(baseline.state === 'EDITABLE' && baseline.snapshot_id === config.restoreSnapshotId && !admin.locks.length, 'NAMED_UNLOCKED_EDITABLE_BASELINE_REQUIRED')
      need(baseline.cooldown_ms >= 5000 && baseline.cooldown_ms <= config.maxCooldownMs, 'REAL_NONZERO_BOUNDED_COOLDOWN_REQUIRED')
      for (let n = 0; n < 2; n++) {
        const deniedAdmin = await member(n).call('getConcertAdminMural')
        assert.equal(deniedAdmin.status, 403); assert.equal(reason(deniedAdmin.value), 'PERMISSION_DENIED')
        const capability = await tools(member(n), baseline.revision)
        need(capability.background_index === 0 && capability.editable && capability.remaining_cells === 512 && !capability.latest_action && !capability.unresolved, 'PROVISIONED_CURRENT_WALL_B_REQUIRED')
      }
      await converge()
    })
    let pencil: OwnAction, fill: OwnAction, preFill: Snapshot
    await step('pencil', async () => { const s = await ready(0); pencil = await uiAction(0, paint(s, 1, 1), s) })
    await step('sharedCooldown', async () => {
      const s = await fresh(), before = await tools(member(0))
      const wait = Date.parse(pencil.next_allowed_at) - Date.parse(s.server_time)
      need(wait > 0, 'COOLDOWN_ALREADY_ELAPSED_NO_EVIDENCE')
      await Promise.all([denied(0, paint(s, 2, 1), 'MURAL_COOLDOWN'), denied(0, { operation_id: 'eraseConcertMuralPixel', key: randomUUID(), body: { expected_revision: s.revision, x: 1, y: 1 } }, 'MURAL_COOLDOWN')])
      assert.equal((await tools(member(0))).remaining_cells, before.remaining_cells)
    })
    await step('opaqueErase', async () => {
      const s = await ready(0)
      // Pencil may choose index0; select a genuine non-background cell instead.
      const seed = s.pixels[129] ? 129 : boundedSeed(s, 256)
      await uiAction(0, { operation_id: 'eraseConcertMuralPixel', key: randomUUID(), body: { expected_revision: s.revision, x: seed % 128, y: Math.floor(seed / 128) } }, s)
    })
    await step('boundedFill', async () => {
      preFill = await ready(0); const budget = await tools(member(0)), seed = boundedSeed(preFill, Math.floor((budget.remaining_cells - 10) / 4))
      const cells = connectedRegion(preFill, seed); need(cells.length > 0 && cells.length <= 256, 'ACTUAL_BOUNDED_REGION_REQUIRED')
      fill = await uiAction(0, { operation_id: 'fillConcertMuralRegion', key: randomUUID(), body: { expected_revision: preFill.revision, x: seed % 128, y: Math.floor(seed / 128), color_index: (preFill.pixels[seed]! + 1) % 16 } }, preFill)
    })
    await step('eligibleUndo', async () => {
      const s = await ready(0), t = await tools(member(0), s.revision)
      need(t.undo?.action_id === fill.action_id && t.available.undo, 'CURRENT_ELIGIBLE_OWN_LAST_ACTION_REQUIRED')
      await uiAction(0, { operation_id: 'undoConcertMuralAction', key: randomUUID(), body: { expected_revision: s.revision, target_action_id: fill.action_id } }, s, preFill)
    })
    await step('reloadBudget', async () => {
      const before = await tools(member(0)); await page(0).reload()
      await page(0).getByRole('region', { name: 'Wall editor' }).waitFor()
      const after = await tools(member(0)); assert.equal(after.remaining_cells, before.remaining_cells)
      assert.equal(after.latest_action?.action_id, before.latest_action?.action_id); assert.equal(after.undo, null)
      await converge()
    })
    await step('remoteInvalidatesUndo', async () => {
      const own = await ready(0), action = await uiAction(0, paint(own, 3, 3), own)
      const remote = await ready(1); await uiAction(1, paint(remote, 4, 3), remote)
      const s = await ready(0); assert.equal((await tools(member(0))).undo, null)
      await denied(0, { operation_id: 'undoConcertMuralAction', key: randomUUID(), body: { expected_revision: s.revision, target_action_id: action.action_id } }, 'MURAL_UNDO_NOT_LATEST')
    })
    await step('lostActionRecovery', async () => {
      const s = await ready(0), seed = boundedSeed(s, 32)
      await uiAction(0, { operation_id: 'fillConcertMuralRegion', key: randomUUID(), body: { expected_revision: s.revision, x: seed % 128, y: Math.floor(seed / 128), color_index: (s.pixels[seed]! + 1) % 16 } }, s, undefined, true)
    })
    await step('lostPencilRecovery', async () => { const s = await ready(0); await uiAction(0, paint(s, 5, 3), s, undefined, true) })
    await step('privateResults', async () => {
      for (const [key, original] of retained) {
        const foreign = await member(1 - original.actor).call('getOwnConcertMuralAction', undefined, { idempotencyKey: key })
        assert.equal(foreign.status, 404); assert.equal(reason(foreign.value), 'RESOURCE_UNAVAILABLE')
      }
      const delta = await member(1).delta(baseline.revision)
      for (const op of delta.operations) { assert(!('actor_id' in op)); assert(!('next_allowed_at' in op)); assert(!('before_colors' in op)) }
    })
    await step('unauthorizedModeration', async () => {
      const before = await operator().admin()
      for (const [id, body] of [
        ['controlConcertMural', { expected_revision: before.revision, action: 'PAUSE', reason: 'IN04 denial' }],
        ['lockConcertMuralRegion', { expected_revision: before.revision, x_min: 10, y_min: 10, x_max_exclusive: 11, y_max_exclusive: 11, locked: true, reason: 'IN04 denial' }],
        ['restoreConcertMural', { expected_revision: before.revision, snapshot_id: config.restoreSnapshotId, reason: 'IN04 denial' }],
      ] as const) {
        const r = await member(0).call(id, body); assert.equal(r.status, 403); assert.equal(reason(r.value), 'PERMISSION_DENIED')
      }
      assert.deepEqual(await operator().admin(), before)
    })
    await step('pause', async () => {
      await control('PAUSE'); const s = await converge()
      await denied(0, paint(s, 8, 3), 'MURAL_PAUSED')
      await denied(1, { operation_id: 'fillConcertMuralRegion', key: randomUUID(), body: { expected_revision: s.revision, x: 1, y: 1, color_index: 2 } }, 'MURAL_PAUSED')
      await control('RESUME'); await converge()
    })
    const lock = async (locked: boolean) => {
      await alive(); const state = await operator().admin()
      const r = await operator().call('lockConcertMuralRegion', { expected_revision: state.revision, x_min: 10, y_min: 10, x_max_exclusive: 11, y_max_exclusive: 11, locked, reason: 'IN04 Wall B lock' })
      assert.equal(r.status, 200); operator().contracts.decode('Domain_mural_MuralAdminState', r.value)
    }
    await step('regionLock', async () => {
      await ready(0); await lock(true); const s = await converge()
      const rejected = paint(s, 10, 10), path = member(0).contracts.path(rejected.operation_id, config.eventId)
      await preview(page(0), rejected)
      const response = page(0).waitForResponse(r => r.request().method() === 'POST' && new URL(r.url()).pathname === path)
      await page(0).getByRole('button', { name: 'Confirm pixel', exact: true }).click()
      const result = await response; assert.equal(result.status(), 409)
      assert.deepEqual(result.request().postDataJSON(), rejected.body); assert.equal(reason(await result.json()), 'MURAL_REGION_LOCKED')
      await expect(page(0).getByRole('button', { name: 'Confirm pixel', exact: true })).toHaveCount(0)
      await converge(s.pixels)
      await denied(0, { operation_id: 'eraseConcertMuralPixel', key: randomUUID(), body: { expected_revision: s.revision, x: 10, y: 10 } }, 'MURAL_REGION_LOCKED')
      await lock(false); const unlocked = await ready(0); await uiAction(0, paint(unlocked, 10, 10), unlocked)
    })
    const restore = async () => {
      await alive(); const before = await fresh(), admin = await operator().admin()
      const r = await operator().call('restoreConcertMural', { expected_revision: admin.revision, snapshot_id: config.restoreSnapshotId, reason: 'IN04 named baseline restore' })
      assert.equal(r.status, 200); const state = operator().contracts.decode<Admin>('Domain_mural_MuralAdminState', r.value)
      assert(state.revision > before.revision); assert.notEqual(state.snapshot_id, before.snapshot_id)
      return state
    }
    await step('restore', async () => { const state = await restore(); const s = await converge(baseline.pixels); assert.equal(s.revision, state.revision); assert.equal((await tools(member(0))).undo, null) })
    await step('obsoleteDelta', async () => {
      // Delay actual old response bytes only; never fabricate a delta/reset/ACK.
      const path = member(1).contracts.path('getConcertMuralDelta', config.eventId)
      let release: (() => Promise<void>) | undefined, abort: (() => Promise<void>) | undefined
      await page(1).route(config.origin + path + '*', async route => {
        const response = await route.fetch({ maxRetries: 0, timeout: 15000 })
        if (release) { await route.fulfill({ response }); return }
        const bytes = await response.body()
        release = () => route.fulfill({ response, body: bytes }); abort = () => route.abort()
      })
      try {
        await expect.poll(() => !!release).toBe(true)
        await restore()
        const refresh = page(1).getByRole('button', { name: 'Refresh confirmed wall', exact: true })
        await refresh.click(); await release!(); release = undefined; abort = undefined
        await converge(baseline.pixels)
      } finally { await abort?.().catch(() => {}); await page(1).unroute(config.origin + path + '*') }
    })
    await step('reconnect', async () => {
      const before = captureReconnectBefore(observers[1]!)
      await contexts[1]!.setOffline(true)
      try { await expect.poll(() => observers[1]!.active).toBe(0); const s = await ready(0); await uiAction(0, paint(s, 12, 10), s, undefined, false, false) }
      finally { await contexts[1]!.setOffline(false) }
      await reconnectWorldOnce(page(1), observers[1]!, before, false, Math.min(deadline, Date.now() + 15000))
      await converge()
    })
    await step('restartEpoch', async () => {
      const old = observers.map(captureReconnectBefore)
      const started = Date.now(), proof = await port.observeRestart(config, { roomEpoch: old[0]!.roomEpoch, streamId: old[0]!.streamId })
      need(proof.candidate === config.releaseRevision && proof.environmentId === config.environmentId && Date.parse(proof.restartedAt) > started && Date.parse(proof.restartedAt) <= Date.now(), 'CURRENT_EXTERNAL_RESTART_PROOF_REQUIRED')
      const reconnectDeadline = Math.min(deadline, Date.now() + config.restartObservationTimeoutMs)
      for (let n = 0; n < 2; n++) await reconnectWorldOnce(page(n), observers[n]!, old[n]!, true, reconnectDeadline)
      await expect.poll(() => observers.every((o, n) => o.baseline && o.baseline.roomEpoch !== old[n]!.roomEpoch && o.baseline.streamId !== old[n]!.streamId), { timeout: Math.max(1, reconnectDeadline - Date.now()) }).toBe(true)
      await converge()
    })
    await step('finalFreeze', async () => {
      await control('FREEZE'); const s = await converge(); assert.equal(s.state, 'FROZEN')
      for (let n = 0; n < 2; n++) {
        await denied(n, paint(s, 13, 10), 'MURAL_FROZEN')
        await denied(n, { operation_id: 'fillConcertMuralRegion', key: randomUUID(), body: { expected_revision: s.revision, x: 1, y: 1, color_index: 2 } }, 'MURAL_FROZEN')
        await expect(page(n).getByRole('button', { name: 'Fill preview', exact: true })).toBeDisabled()
      }
      report.finalState = 'FROZEN_OWNED_ISOLATED_EVENT'
    })
  } catch (error) {
    checks[current] = { status: error instanceof NotRun ? 'NOT_RUN' : 'FAIL', code: error instanceof NotRun ? error.code : 'ASSERTION_OR_RUNTIME_FAILURE' }
    if (!(error instanceof NotRun)) failures.push(current)
  } finally {
    if (timer) clearTimeout(timer)
    try {
      const results = await Promise.allSettled(contexts.map(c => c.close()))
      try { await browser?.close() } finally { actors.forEach(a => a.clear()) }
      assert(results.every(r => r.status === 'fulfilled'))
      assert(observers.every(o => o.active === 0)); assert.equal(pageErrors.length, 0)
      checks.teardown = contexts.length ? { status: 'PASS' } : { status: 'NOT_RUN', code: 'NO_BROWSER_ALLOCATED' }
    } catch { checks.teardown = { status: 'FAIL', code: 'CLEANUP_OR_PRODUCT_PAGE_ERROR' } }
    report.status = Object.values(checks).some(v => v.status === 'FAIL') ? 'FAIL' : Object.values(checks).every(v => v.status === 'PASS') ? 'PASS' : 'NOT_RUN'
    // Credentials, tokens, headers, raw page errors and private deployment payloads are never serialized.
    const path = info.outputPath('in04-wall-b-result.json')
    await writeFile(path, JSON.stringify(report, null, 2) + '\n', { mode: 0o600 })
  }
  if (report.status !== 'PASS') throw Error(`IN04 ${report.status}: ${failures.join(',') || checks[current].code || 'UNEXECUTED_REQUIREMENTS'}`)
})
