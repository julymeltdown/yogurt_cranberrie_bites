// FE10 phase-two attachment to a Root-owned actual App page. No server, browser,
// context, Auth factory, credentials, synthetic frame or application API is made.
import assert from 'node:assert/strict'
import { setTimeout as delay } from 'node:timers/promises'

export const worldObserverSource = '0de2faa3f7977a1a60981b5ae7c1208675680abf'

export async function attachEnrollmentWorldObservation({ page, origin, eventId, actorId, maximumPlayers,
  phaserModuleUrl, observerSourceRevision, observeWorldPage, inspectActualRenderer, guard, signal, expiresAt }) {
  assert.equal(observerSourceRevision, worldObserverSource, 'Root must supply the exact read-only observer source and retain its byte receipts')
  assert.equal(typeof observeWorldPage, 'function'); assert.equal(typeof inspectActualRenderer, 'function')
  assert.equal(typeof guard, 'function'); assert(signal instanceof AbortSignal)
  assert.equal(new URL(origin).origin, origin)
  const moduleUrl = new URL(phaserModuleUrl)
  assert.equal(moduleUrl.origin, origin)
  assert(moduleUrl.pathname.startsWith('/@fs/') && moduleUrl.pathname.endsWith('/deps/phaser.js'))
  assert(/^\?v=[a-f0-9]+$/.test(moduleUrl.search) && !moduleUrl.hash && !moduleUrl.username && !moduleUrl.password)
  assert(page.url() === 'about:blank', 'Attach before first navigation; an existing World baseline cannot be backfilled')
  assert(Number.isFinite(Date.parse(expiresAt)))
  signal.throwIfAborted(); await guard(); signal.throwIfAborted()
  assert(Date.parse(expiresAt) - Date.now() > 5000, 'FE10_WORLD_WINDOW_EXPIRED')
  const observer = observeWorldPage(page, { eventId, actorId, maximumPlayers, origin, maximumQueuedFrames: 32 })
  let cdp, game, retired = false, pageErrors = 0, connectedChecks = 0, identityChecks = 0, releaseVerified = false
  const onError = () => { pageErrors++ }
  page.on('pageerror', onError)
  async function check() {
    signal.throwIfAborted()
    assert(!retired && Date.parse(expiresAt) - Date.now() > 5000, 'FE10_WORLD_WINDOW_EXPIRED')
    await guard()
    assert.equal(pageErrors, 0, 'Actual App page error; private details are not retained')
  }
  async function until(read) {
    const deadline = Math.min(Date.now() + 15000, Date.parse(expiresAt) - 5000)
    while (Date.now() < deadline) {
      await check()
      const value = await read()
      if (value) return value
      await delay(100, undefined, { signal })
    }
    throw new Error('FE10_ACTUAL_WORLD_OBSERVATION_TIMEOUT')
  }
  // Retain an actual Game object reference in CDP. Counts alone do not establish
  // identity. Each later comparison queries the current actual Game collection.
  async function actualGame(expected) {
    cdp ??= await page.context().newCDPSession(page)
    const temporary = []
    try {
      const prototype = await cdp.send('Runtime.evaluate', {
        expression: 'import(' + JSON.stringify(moduleUrl.href) + ').then(m => (m.default ?? m).Game.prototype)', awaitPromise: true,
      })
      assert(!prototype.exceptionDetails && prototype.result.objectId, 'Actual Phaser prototype unavailable')
      temporary.push(prototype.result.objectId)
      const instances = await cdp.send('Runtime.queryObjects', { prototypeObjectId: prototype.result.objectId })
      assert(instances.objects.objectId); temporary.push(instances.objects.objectId)
      const result = await cdp.send('Runtime.callFunctionOn', {
        objectId: instances.objects.objectId,
        functionDeclaration: `function (expected) {
          const canvas = document.querySelector('.shell-world-host canvas')
          const games = this.filter(value => value.isRunning && !value.pendingDestroy)
          if (games.length !== 1 || games[0].canvas !== canvas) return false
          return expected ? games[0] === expected : games[0]
        }`,
        ...(expected ? { arguments: [{ objectId: expected }], returnByValue: true } : {}),
      })
      assert(!result.exceptionDetails, 'Actual Game reference observation failed')
      if (expected) return result.result.value === true
      assert(result.result.objectId, 'Exactly one owned actual Game is required')
      return result.result.objectId
    } finally {
      for (const objectId of temporary) await cdp.send('Runtime.releaseObject', { objectId }).catch(() => {})
    }
  }
  async function connected() {
    const sample = await until(async () => {
      await observer.drain()
      const state = observer.observation.read(), counts = observer.summary()
      assert.equal(counts.invalid + counts.foreignSnapshots + counts.pendingOverflow, 0)
      assert.equal(counts.peakSockets <= 1, true, 'More than one concurrent actual World socket')
      if (!state.baseline || state.activeSockets !== 1) return null
      assert.equal(state.baseline.eventId, eventId); assert.equal(state.baseline.selfUserId, actorId)
      assert(state.players.has(actorId))
      const renderer = await inspectActualRenderer(page, moduleUrl.href)
      if (renderer.games !== 1 || renderer.canvases !== 1 || renderer.sprites.length !== state.players.size) return null
      const ordered = values => [...values].map(value => [value.x, value.y]).sort((a, b) => a[0] - b[0] || a[1] - b[1])
      if (JSON.stringify(ordered(renderer.sprites)) !== JSON.stringify(ordered(state.players.values()))) return null
      return { state, counts }
    })
    if (!game) game = await actualGame()
    assert(await actualGame(game), 'Actual Game was replaced')
    connectedChecks++
    return sample
  }
  async function retainsAcrossPrivateNavigation(action) {
    assert.equal(typeof action, 'function')
    const before = await connected()
    // Caller performs ordinary existing UI navigation. No page.goto, injected
    // router state, visibility override or hidden private application command.
    await check(); await action(); await check()
    const after = await connected()
    assert.equal(after.counts.opens, before.counts.opens, 'Navigation opened another World connection')
    assert.equal(after.state.baseline.streamId, before.state.baseline.streamId, 'Navigation replaced the current World stream')
    assert.equal(after.state.baseline.roomEpoch, before.state.baseline.roomEpoch)
    assert.equal(after.state.baseline.roomId, before.state.baseline.roomId)
    assert(await actualGame(game), 'Private navigation replaced the actual Game')
    identityChecks++
  }
  async function releasedAfterActualLogout() {
    // Root/Auth owner performs real UI logout first and retains its HTTP receipt.
    // Browser absence does not certify backend membership cleanup by itself.
    await until(async () => {
      await observer.drain()
      const state = observer.observation.read()
      if (state.activeSockets !== 0 || state.baseline !== null) return false
      const renderer = await inspectActualRenderer(page, moduleUrl.href)
      return renderer.games === 0 && renderer.canvases === 0
    })
    if (game) { await cdp.send('Runtime.releaseObject', { objectId: game }); game = undefined }
    releaseVerified = true
  }
  async function dispose() {
    if (retired) return
    retired = true; page.off('pageerror', onError)
    try { await observer.dispose() } finally {
      if (game) await cdp?.send('Runtime.releaseObject', { objectId: game }).catch(() => {})
      game = undefined; await cdp?.detach()
    }
  }
  return { connected, retainsAcrossPrivateNavigation, releasedAfterActualLogout, dispose,
    summary: () => ({ observer: observer.summary(), connectedChecks, actualGameIdentityChecks: identityChecks,
      browserReleaseVerified: releaseVerified, pageErrors,
      qualification: 'World observation attachment only. Actual Auth/admission/bootstrap/current-user and backend membership receipts remain required. No per-sprite identity or art acceptance.' }) }
}
