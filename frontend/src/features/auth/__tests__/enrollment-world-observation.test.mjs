import test from 'node:test'
import assert from 'node:assert/strict'
import { attachEnrollmentWorldObservation, worldObserverSource } from './enrollment-world-observation.mjs'

function fixture() {
  let game = 'original-game', retired = false, released = false
  const releasedObjects = [], counts = { opens: 1, peakSockets: 1, invalid: 0, foreignSnapshots: 0, pendingOverflow: 0 }
  const state = { baseline: { eventId: 'event', selfUserId: 'actor', streamId: 'stream', roomId: 'room', roomEpoch: 1 },
    activeSockets: 1, players: new Map([['actor', { x: 1, y: 2 }]]) }
  const cdp = { async send(method, args) {
    if (method === 'Runtime.evaluate') return { result: { objectId: 'prototype' } }
    if (method === 'Runtime.queryObjects') return { objects: { objectId: 'instances' } }
    if (method === 'Runtime.callFunctionOn') return { result: args.arguments ? { value: args.arguments[0].objectId === game } : { objectId: game } }
    if (method === 'Runtime.releaseObject') { releasedObjects.push(args.objectId); return {} }
    throw new Error('Unexpected CDP operation')
  }, async detach() { retired = true } }
  const observer = { observation: { read: () => state }, summary: () => ({ ...counts }), async drain() {}, async dispose() {} }
  const input = { page: { url: () => 'about:blank', on() {}, off() {}, context: () => ({ newCDPSession: async () => cdp }) },
    origin: 'http://localhost:3000', eventId: 'event', actorId: 'actor', maximumPlayers: 2,
    phaserModuleUrl: 'http://localhost:3000/@fs/allocated/deps/phaser.js?v=abc123',
    observerSourceRevision: worldObserverSource, observeWorldPage: () => observer,
    inspectActualRenderer: async () => released ? { games: 0, canvases: 0, sprites: [] } : { games: 1, canvases: 1, sprites: [{ x: 1, y: 2 }] },
    guard: async () => {}, signal: new AbortController().signal, expiresAt: new Date(Date.now() + 60000).toISOString() }
  return { input, counts, state, releasedObjects, retired: () => retired,
    replaceGame() { game = 'replacement-game' }, release() { released = true; state.activeSockets = 0; state.baseline = null } }
}

test('one Game and canvas count cannot conceal replacement across private navigation', async () => {
  const f = fixture(), attachment = await attachEnrollmentWorldObservation(f.input)
  try {
    await attachment.connected()
    await assert.rejects(attachment.retainsAcrossPrivateNavigation(async () => f.replaceGame()), /Game was replaced/)
    assert.equal(attachment.summary().actualGameIdentityChecks, 0)
  } finally { await attachment.dispose() }
  assert(f.releasedObjects.includes('original-game')); assert(f.retired())
})

test('a quiet current socket cannot conceal an extra sequential connection', async () => {
  const f = fixture(), attachment = await attachEnrollmentWorldObservation(f.input)
  try {
    await assert.rejects(attachment.retainsAcrossPrivateNavigation(async () => { f.counts.opens++ }), /another World connection/)
  } finally { await attachment.dispose() }
})

test('actual Game reference survives navigation and is released after observed logout cleanup', async () => {
  const f = fixture(), attachment = await attachEnrollmentWorldObservation(f.input)
  try {
    await attachment.retainsAcrossPrivateNavigation(async () => {})
    assert.equal(attachment.summary().actualGameIdentityChecks, 1)
    f.release(); await attachment.releasedAfterActualLogout()
    assert.equal(attachment.summary().browserReleaseVerified, true)
    assert(f.releasedObjects.includes('original-game'))
  } finally { await attachment.dispose() }
})

test('an expired allocation never installs observation listeners', async () => {
  const f = fixture(); let attached = false
  f.input.expiresAt = new Date(Date.now() - 1).toISOString()
  f.input.observeWorldPage = () => { attached = true; throw new Error('Must not attach') }
  await assert.rejects(attachEnrollmentWorldObservation(f.input), /WINDOW_EXPIRED/)
  assert.equal(attached, false)
})

test('canonical observer provenance mismatch fails before creating a CDP observer', async () => {
  const f = fixture(); f.input.observerSourceRevision = 'unmatched-source'
  await assert.rejects(attachEnrollmentWorldObservation(f.input), /exact read-only observer source/)
  assert.equal(f.retired(), false)
})
