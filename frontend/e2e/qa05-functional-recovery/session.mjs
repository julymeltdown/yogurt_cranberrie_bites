// Actual default App attachment only. Root supplies its already managed origin,
// exact Phaser module, two real principals and fresh resource guard. No server,
// credentials provisioning, transport replacement or private App API is created.
import assert from 'node:assert/strict'
import { setTimeout as delay } from 'node:timers/promises'
import { observeWorldPage, inspectActualRenderer } from './browser-observer.mjs'
import { assertPairConverged } from './observations.mjs'

export async function attachAppPair({ browser, binding, origin, phaserModuleUrl, principals, secondRoom, guard, signal }) {
  assert.equal(typeof guard, 'function', 'A current independently checked allocation is required')
  assert(signal instanceof AbortSignal, 'The owning window must supply its abort signal')
  assert.equal(new URL(origin).origin, origin)
  assert.equal(new URL(phaserModuleUrl).origin, origin)
  assert.equal(principals.length, 2); assert.notEqual(principals[0].id, principals[1].id)
  assert(principals.every(p => ['id', 'email', 'password'].every(key => typeof p[key] === 'string' && p[key].length > 0)))
  const contexts = [], pages = [], observers = [], visibility = []
  let visibilityHandle, retired = false, pageErrors = 0
  // The native browser must be a fresh dedicated profile connected using
  // noDefaults:true. The default context is needed for trusted tab visibility;
  // the second principal gets separate cookies/storage, never a second Auth ID.
  const nativeContext = browser.contexts()[0]
  assert(nativeContext && browser.contexts().length === 1 && (await nativeContext.cookies()).length === 0, 'Fresh dedicated native profile required')
  assert(nativeContext.pages().every(page => page.url() === 'about:blank'), 'Do not attach to personal browser tabs')
  async function check() {
    signal.throwIfAborted()
    assert(!retired && Date.parse(binding.expiresAt) - Date.now() > 5000, 'QA05_APP_WINDOW_EXPIRED')
    await guard()
    assert.equal(pageErrors, 0, 'Actual App page error; details are not persisted')
  }
  async function until(read, maximumMs = 15000) {
    const end = Math.min(Date.now() + maximumMs, Date.parse(binding.expiresAt) - 5000)
    while (Date.now() < end) {
      await check()
      const value = await read()
      if (value) return value
      await delay(100, undefined, { signal })
    }
    throw new Error('QA05_ACTUAL_APP_CONVERGENCE_TIMEOUT')
  }
  async function sample() {
    await check()
    await Promise.all(observers.map(o => o.drain()))
    return { observations: observers.map(o => o.observation), renderings: await Promise.all(pages.map(page => inspectActualRenderer(page, phaserModuleUrl))) }
  }
  function progressed(caseId, beforeState, current) {
    const states = current.observations.map(o => o.read()), counts = current.observations.map(o => o.summary())
    if (states.some(s => !s.baseline)) return false
    if (caseId === 'packet-gap') return counts[0].gaps > beforeState[0].counts.gaps && counts[0].snapshots > beforeState[0].counts.snapshots
    if (caseId === 'foreground') return counts[0].opens > beforeState[0].counts.opens && states[0].baseline.streamId !== beforeState[0].baseline.streamId
    if (caseId === 'world-restart') return states.every((s, i) => s.baseline.streamId !== beforeState[i].baseline.streamId && s.baseline.roomEpoch !== beforeState[i].baseline.roomEpoch)
    if (caseId === 'room-switch') return states.every((s, i) => s.baseline.roomId === secondRoom.id && s.baseline.roomId !== beforeState[i].baseline.roomId)
    if (caseId === 'packet-delay' || caseId === 'slow-client') return counts[0].inputs > beforeState[0].counts.inputs && counts[0].deltas > beforeState[0].counts.deltas && states[0].pending === 0
    // Replay rejection and unchanged backend membership require actual owner
    // proof at after-observation. A quiet browser alone cannot prove the fault.
    return caseId === 'ticket-replay'
  }
  async function converge({ caseId, beforeState } = {}) {
    return until(async () => {
      const current = await sample()
      if (caseId && !progressed(caseId, beforeState, current)) return null
      try { assertPairConverged(current.observations, current.renderings) } catch { return null }
      if (visibilityHandle) {
        visibility.splice(0, visibility.length, ...await visibilityHandle.evaluate(value => value.read()))
      }
      return { ...current, trustedVisibility: visibility.map(v => ({ ...v })) }
    }, 25000)
  }
  async function reconnect() {
    // Called only after actual producer-ready. Never retry the action blindly.
    for (const page of pages) {
      await check()
      const button = page.getByRole('region', { name: 'World connection status', exact: true }).getByRole('button', { name: 'Reconnect', exact: true })
      await button.waitFor({ state: 'visible', timeout: 5000 })
      await button.click()
    }
  }
  async function move() {
    await check()
    const page = pages[0], button = page.getByRole('button', { name: 'Movement joystick. Arrow keys or W A S D when focused', exact: true })
    await page.bringToFront(); await button.focus()
    // Ordinary keyboard input through the actual joystick produces genuine
    // deltas after the owner has armed interception; no fabricated WS frames.
    for (const key of ['ArrowRight', 'ArrowDown']) {
      await page.keyboard.down(key)
      try { await delay(350, undefined, { signal }); await check() }
      finally { await page.keyboard.up(key) }
    }
  }
  async function foreground() {
    await check(); await visibilityHandle.evaluate(value => value.clear())
    const cover = await nativeContext.newPage()
    try {
      await cover.goto('about:blank'); await cover.bringToFront()
      await until(async () => {
        const events = await visibilityHandle.evaluate(value => value.read())
        await observers[0].drain()
        return events.some(v => v.hidden && v.trusted) && observers[0].observation.read().activeSockets === 0
      }, 5000)
    } finally { await cover.close(); await pages[0].bringToFront() }
  }
  async function switchRoom() {
    assert(secondRoom && typeof secondRoom.id === 'string' && typeof secondRoom.label === 'string', 'Actual second available room required')
    for (const page of pages) {
      await check()
      // Shell navigation retains the one persistent Game/connection. A full
      // page.goto would test an unrequested App reload instead of UI switching.
      await page.getByRole('link', { name: 'Rooms', exact: true }).click()
      await page.getByRole('heading', { name: 'Choose your room', exact: true }).waitFor()
      await page.getByRole('button', { name: `Switch to ${secondRoom.label}`, exact: true }).click()
      await page.getByText('Room switch confirmed.', { exact: true }).waitFor()
      await page.getByRole('link', { name: 'Back to world', exact: true }).click()
    }
  }
  async function dispose() {
    if (retired) return
    retired = true
    if (visibilityHandle) {
      await visibilityHandle.evaluate(value => value.dispose()).catch(() => {})
      await visibilityHandle.dispose()
    }
    // Close actual sockets before discarding observation listeners. Root's
    // separate owning membership observation still certifies server cleanup.
    await Promise.all(pages.map(page => page.close()))
    await Promise.all(observers.map(o => o.dispose()))
    await Promise.all(contexts.map(context => context.close()))
  }
  try {
    await check()
    const second = await browser.newContext({ viewport: { width: 390, height: 844 }, serviceWorkers: 'block' })
    contexts.push(second)
    for (const context of [nativeContext, second]) {
      const page = await context.newPage(); pages.push(page)
      await page.setViewportSize({ width: 390, height: 844 })
      page.setDefaultTimeout(10000); page.setDefaultNavigationTimeout(15000)
      page.on('pageerror', () => { pageErrors++ })
      observers.push(observeWorldPage(page, { eventId: binding.eventId, actorId: principals[pages.length - 1].id, maximumPlayers: 2, origin }))
    }
    for (let i = 0; i < 2; i++) {
      await check()
      const page = pages[i], principal = principals[i]
      await page.goto(origin + '/login?returnTo=%2Fworld')
      await page.getByLabel('Email', { exact: true }).fill(principal.email)
      await page.getByLabel('Password', { exact: true }).fill(principal.password)
      const response = page.waitForResponse(r => new URL(r.url()).origin === origin && new URL(r.url()).pathname === '/api/auth/login' && r.request().method() === 'POST')
      await page.getByRole('button', { name: 'Sign in with email', exact: true }).click()
      const auth = await response
      assert(auth.ok(), 'Actual canonical login rejected')
      assert.equal((await auth.json()).userId, principal.id, 'Actual canonical identity differs')
    }
    await pages[0].bringToFront()
    // JSHandle retains only read-only trusted visibility metadata in a closure;
    // no window globals, visibility overrides or synthetic events.
    visibilityHandle = await pages[0].evaluateHandle(() => {
      const events = []
      const listener = event => { if (events.length < 16) events.push({ hidden: document.hidden, trusted: event.isTrusted }) }
      document.addEventListener('visibilitychange', listener)
      return { read: () => events.map(v => ({ ...v })), clear: () => { events.length = 0 }, dispose: () => document.removeEventListener('visibilitychange', listener) }
    })
    await converge()
    return { kind: 'QA05_ACTUAL_DEFAULT_APP_PAIR', sample, move, foreground, switchRoom, reconnect, awaitConvergence: converge,
      summary: () => ({ actors: observers.map(o => o.summary()), pageErrors, qualification: 'Actual attachment only if executed in a qualified owner window; source preparation is NOT_RUN.' }), dispose }
  } catch (error) { await dispose(); throw error }
}
