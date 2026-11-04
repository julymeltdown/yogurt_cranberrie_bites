// Observe the existing default App. No WebSocket constructor/handler replacement,
// injected roster, alternate renderer, server, Auth factory or backend import.
import assert from 'node:assert/strict'
import { createWorldObservation } from './observations.mjs'

export function observeWorldPage(page, { eventId, actorId, maximumPlayers, origin, maximumQueuedFrames = 32 }) {
  assert.equal(new URL(origin).origin, origin)
  assert(Number.isInteger(maximumQueuedFrames) && maximumQueuedFrames >= 1 && maximumQueuedFrames <= 64)
  const observation = createWorldObservation({ eventId, actorId, maximumPlayers })
  const listeners = new Map()
  let tail = Promise.resolve(), queued = 0, peakQueued = 0, retired = false, fault = null
  function enqueue(fn) {
    if (retired || fault) return
    if (queued >= maximumQueuedFrames) { fault = 'QA05_OBSERVATION_QUEUE_OVERFLOW'; return }
    queued++; peakQueued = Math.max(peakQueued, queued)
    tail = tail.then(async () => { if (!retired && !fault) await fn() })
      .catch(() => { fault = 'QA05_ACTUAL_DECODER_OR_OBSERVATION_FAILED' }).finally(() => { queued-- })
  }
  async function decode(socket, direction, payload) {
    assert(typeof payload === 'string')
    assert(Buffer.byteLength(payload) <= (direction === 'client' ? 4096 : 65536))
    // Read the exact module served by Root's pinned canonical Vite owner. Decode
    // in its own browser realm, where the actual dependencies already resolve.
    // Authentication values are not returned out of that realm.
    const value = await page.evaluate(async ({ direction, payload }) => {
      const { decode } = await import('/src/world/network/transport/contract.ts')
      const message = decode(payload, direction)
      return message.type === 'authenticate' ? { type: 'authenticate' } : message
    }, { direction, payload })
    observation.message(socket, direction, value)
  }
  function connected(socket) {
    const url = new URL(socket.url())
    if (url.pathname !== '/world-ws' || url.host !== new URL(origin).host) return
    const sent = ({ payload }) => enqueue(() => decode(socket, 'client', payload))
    const received = ({ payload }) => enqueue(() => decode(socket, 'server', payload))
    const closed = () => enqueue(() => observation.close(socket, null))
    listeners.set(socket, { sent, received, closed })
    enqueue(() => observation.open(socket))
    socket.on('framesent', sent); socket.on('framereceived', received); socket.on('close', closed)
  }
  page.on('websocket', connected)
  return {
    observation,
    async drain() { await tail; assert(!fault, fault); return observation.read() },
    summary() { return { ...observation.summary(), queuedFrames: queued, peakQueuedFrames: peakQueued, observationFault: fault } },
    async dispose() {
      retired = true; page.off('websocket', connected)
      for (const [socket, value] of listeners) {
        socket.off('framesent', value.sent); socket.off('framereceived', value.received); socket.off('close', value.closed)
      }
      listeners.clear(); await tail; observation.dispose()
    },
  }
}

export async function inspectActualRenderer(page, phaserModuleUrl) {
  const url = new URL(phaserModuleUrl)
  assert.equal(url.origin, new URL(page.url()).origin)
  assert(url.pathname.startsWith('/@fs/') && url.pathname.endsWith('/deps/phaser.js'))
  assert(/^\?v=[a-f0-9]+$/.test(url.search) && !url.hash && !url.username && !url.password)
  const cdp = await page.context().newCDPSession(page)
  const objects = []
  try {
    // Adapted from the published IN01 read-only heap observation. Resolve the
    // exact versioned module supplied by Root; an unversioned import is wrong.
    const prototype = await cdp.send('Runtime.evaluate', { expression: 'import(' + JSON.stringify(url.href) + ').then(m => (m.default ?? m).Game.prototype)', awaitPromise: true })
    assert(!prototype.exceptionDetails && prototype.result.objectId, 'Actual Phaser prototype unavailable')
    objects.push(prototype.result.objectId)
    const instances = await cdp.send('Runtime.queryObjects', { prototypeObjectId: prototype.result.objectId })
    objects.push(instances.objects.objectId)
    const result = await cdp.send('Runtime.callFunctionOn', { objectId: instances.objects.objectId,
      functionDeclaration: `function () {
        const canvas = document.querySelector('.shell-world-host canvas')
        const games = this.filter(game => game.isRunning && !game.pendingDestroy)
        const owned = games.filter(game => game.canvas === canvas)
        const scene = owned.length === 1 ? owned[0].scene.getScenes(true).find(scene => scene.sys.settings.key === 'mcr-map') : null
        return { games: games.length, canvases: document.querySelectorAll('.shell-world-host canvas').length,
          sprites: scene ? scene.children.list.filter(child => child.active && child.visible && child.type === 'Sprite' && child.texture?.key.startsWith('mcr-avatar-')).map(child => ({ x: child.x, y: child.y })) : [] }
      }`, returnByValue: true })
    assert(!result.exceptionDetails && result.result.value, 'Actual renderer observation unavailable')
    return result.result.value
  } finally {
    for (const objectId of objects) await cdp.send('Runtime.releaseObject', { objectId }).catch(() => {})
    await cdp.detach()
  }
}
