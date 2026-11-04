import assert from 'node:assert/strict'
import { expect } from '@playwright/test'
import type { Page } from '@playwright/test'
import type { WorldObserver } from '../real/world-gate/observer'

type Observation = Pick<WorldObserver, 'baseline' | 'opens' | 'active' | 'peak' | 'invalidFrames'>
export interface ReconnectBefore {
  readonly roomEpoch: string; readonly streamId: string
  readonly eventId: string; readonly selfUserId: string; readonly opens: number
}
export function captureReconnectBefore(observer: Observation): ReconnectBefore {
  assert(observer.baseline && observer.active === 1 && observer.peak === 1 && observer.invalidFrames === 0)
  const { roomEpoch, streamId, eventId, selfUserId } = observer.baseline
  return Object.freeze({ roomEpoch, streamId, eventId, selfUserId, opens: observer.opens })
}
export function requireReplacement(before: ReconnectBefore, observer: Observation, restarted: boolean) {
  assert(observer.active === 1 && observer.peak === 1 && observer.invalidFrames === 0)
  assert.equal(observer.opens, before.opens + 1)
  assert(observer.baseline)
  assert.equal(observer.baseline.eventId, before.eventId)
  assert.equal(observer.baseline.selfUserId, before.selfUserId)
  assert.notEqual(observer.baseline.streamId, before.streamId)
  if (restarted) assert.notEqual(observer.baseline.roomEpoch, before.roomEpoch)
  else assert.equal(observer.baseline.roomEpoch, before.roomEpoch)
}

/** Actual shell links only: a Wall panel makes the World host inert, and the
 * full-panel CSS hides it. Neither hidden controls nor optional retries qualify.
 * Restart authority stays outside this function and outside the QA runtime port.
 */
export async function reconnectWorldOnce(page: Page, observer: Observation, before: ReconnectBefore, restarted: boolean, deadline: number) {
  const remaining = () => { const ms = deadline - Date.now(); assert(ms > 0, 'RECONNECT_WINDOW_EXPIRED'); return ms }
  const original = await page.locator('[data-world-host] canvas').elementHandle()
  assert(original, 'ORIGINAL_WORLD_CANVAS_REQUIRED')
  const sameCanvas = async () => {
    assert.equal(await page.locator('[data-world-host] canvas').count(), 1)
    assert(await original.evaluate(node => node.isConnected && node === document.querySelector('[data-world-host] canvas')), 'PERSISTENT_WORLD_CANVAS_REQUIRED')
  }
  const navigate = async (name: 'World' | 'Wall', path: '/world' | '/wall') => {
    const link = page.getByRole('navigation', { name: 'Main navigation', exact: true }).getByRole('link', { name, exact: true })
    await link.waitFor({ state: 'visible', timeout: remaining() })
    assert(await link.isEnabled(), 'ENABLED_SHELL_NAVIGATION_REQUIRED')
    await link.click({ timeout: remaining() })
    await expect.poll(() => new URL(page.url()).pathname, { timeout: remaining() }).toBe(path)
  }
  try {
    await navigate('World', '/world')
    await expect.poll(async () => await page.locator('.world-shell').getAttribute('data-panel-size') === 'closed'
      && await page.locator('[data-world-host]').getAttribute('inert') === null, { timeout: remaining() }).toBe(true)
    const controls = page.getByRole('region', { name: 'World scene and controls', exact: true })
    await controls.waitFor({ state: 'visible', timeout: remaining() })
    const reconnect = controls.getByRole('button', { name: 'Reconnect world', exact: true })
    await reconnect.waitFor({ state: 'visible', timeout: remaining() })
    assert(await reconnect.isEnabled(), 'ENABLED_EXPLICIT_RECONNECT_REQUIRED')
    await expect.poll(() => observer.active, { timeout: remaining() }).toBe(0)
    assert.equal(observer.opens, before.opens, 'NO_IMPLICIT_RECONNECT_ALLOWED')
    await sameCanvas()
    await reconnect.click({ timeout: remaining() }) // Exactly one user action; never replay.
    await expect.poll(() => {
      try { requireReplacement(before, observer, restarted); return true } catch { return false }
    }, { timeout: remaining() }).toBe(true)
    await sameCanvas()
    await navigate('Wall', '/wall')
    await page.getByRole('region', { name: 'Wall editor', exact: true }).waitFor({ state: 'visible', timeout: remaining() })
    await sameCanvas()
    requireReplacement(before, observer, restarted)
  } finally { await original.dispose() }
}
