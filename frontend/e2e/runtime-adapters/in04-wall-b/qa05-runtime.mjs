// Root-owned in-process attachment; provisions no actor, service or window.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { isDeepStrictEqual as equal } from 'node:util'
import { sha256 } from './pinned-source.mjs'

const prefix = 'frontend/e2e/runtime-adapters/in04-wall-b/'
const driver = 'frontend/e2e/qa05-functional-recovery/'
export const qualifiedQa05Components = Object.freeze({
  [prefix + 'qa05-relay.mjs']: 'bb1297acf03dc8f207b76832fbf6454b55fdf812a3e2418b7b2e0376a5f8e0a0',
  [prefix + 'qa05-authentication.mjs']: '2ee000495026cdce8c8af61eb094838470bf86365eede5455a58a38e40e173c9',
  [prefix + 'qa05-ticket-probe.mjs']: '88b4e52d764551f54de1f98e83724d93421dd47980ac33899d6f315e33c474ca',
  [driver + 'checkpoints.mjs']: 'cea269b4a56150087b0370335834cc7a2d595c3320305e40328f4b9b8bafefc2',
  [driver + 'owner-checkpoints.mjs']: '1aac98ec09c2e7d863ba3d5371e1467029d09cbb42bea570ab7f15b06b3890d3',
  [driver + 'frame-faults.mjs']: 'ad76fd4be3702d90676ed5b156c8831e2b5f5f016595f5c8130ec8020e975a05',
  'backend/scripts/mcr/in04_wall_runtime/control.mjs': 'bae80cb67b686097094e921151a8f4f48dc75ed50e11cf72031bc78955088fd7',
})
const dataUrl = bytes => 'data:text/javascript;base64,' + Buffer.from(bytes).toString('base64')
function link(bytes, path, url) {
  const input = bytes.toString(), needle = "from '" + path + "'"
  assert.equal(input.split(needle).length, 2, 'Exactly one pinned relative dependency is required')
  return Buffer.from(input.replace(needle, 'from ' + JSON.stringify(url)))
}

export async function loadQa05Components({ source, backendBytes }) {
  const read = path => {
    const bytes = path.startsWith('backend/') ? backendBytes : source.bytes(path)
    assert.equal(sha256(bytes), qualifiedQa05Components[path], 'Qualified component bytes differ: ' + path)
    return bytes
  }
  const authentication = dataUrl(read(prefix + 'qa05-authentication.mjs'))
  const relayUrl = dataUrl(link(read(prefix + 'qa05-relay.mjs'), './qa05-authentication.mjs', authentication))
  const checkpointUrl = dataUrl(read(driver + 'checkpoints.mjs'))
  const [relay, faults, checkpoints, control, probe] = await Promise.all([
    import(relayUrl), import(dataUrl(read(driver + 'frame-faults.mjs'))),
    import(dataUrl(link(read(driver + 'owner-checkpoints.mjs'), './checkpoints.mjs', checkpointUrl))),
    import(dataUrl(read('backend/scripts/mcr/in04_wall_runtime/control.mjs'))),
    import(dataUrl(link(read(prefix + 'qa05-ticket-probe.mjs'), './qa05-relay.mjs', relayUrl))),
  ])
  return Object.freeze({ installRelay: relay.installQa05WorldRelay, createFaultController: faults.createFrameFaultController,
    createCheckpoints: checkpoints.createOwnerCheckpoints, ownerControl: control.ownerControl,
    createTicketProbe: probe.createConsumedTicketProbe })
}

export function validateQa05Binding({ config, binding, settings, candidate, guard, signal }, now = Date.now()) {
  assert(signal instanceof AbortSignal && !signal.aborted)
  assert.equal(typeof guard, 'function')
  const result = guard(); assert(result !== false && !result?.then)
  assert(equal(Object.keys(binding).sort(), ['candidate', 'environmentId', 'eventId', 'expiresAt', 'windowId']))
  assert(binding.candidate === candidate && config.releaseRevision === candidate)
  assert(config.environmentId === binding.environmentId && config.environmentId === settings.environmentId)
  assert(config.eventId === binding.eventId && config.eventId === settings.eventId)
  assert(config.windowId === binding.windowId && config.origin === settings.origin)
  assert(Number.isFinite(now) && Date.parse(binding.expiresAt) - now > 5000 && Date.parse(binding.expiresAt) - now <= 15 * 60000)
  assert(config.members.length === 2 && new Set(config.members.map(member => member.userId)).size === 2)
  return true
}

export async function attachQa05Runtime({ repo, source, candidate, server, settings, config, binding,
  guard, signal, enableConsumedTicket = false }) {
  const args = { config, binding, settings, candidate, guard, signal }
  validateQa05Binding(args); assert.equal(typeof enableConsumedTicket, 'boolean')
  // The same pinned frontend Git tree supplies all browser and adapter modules.
  // Backend scripts are outside that loader's frontend/contracts allowlist.
  const backendBytes = execFileSync('git', ['show', candidate + ':backend/scripts/mcr/in04_wall_runtime/control.mjs'],
    { cwd: repo, maxBuffer: 512 * 1024 })
  const components = await loadQa05Components({ source, backendBytes })
  const protocol = await server.ssrLoadModule('/src/world/network/transport/contract.ts')
  assert.equal(typeof protocol.decode, 'function'); validateQa05Binding(args)
  const observed = await components.ownerControl.inspect(config)
  const backendBinding = { candidate, environmentId: binding.environmentId, eventId: binding.eventId,
    windowId: binding.windowId, origin: config.origin, domainHash: config.contractHashes.domain,
    realtimeHash: config.contractHashes.realtime, members: config.members.map(member => member.userId) }
  assert(observed.state === 'OBSERVED' && equal(observed.binding, backendBinding) && observed.generation)
  validateQa05Binding(args)
  const lifetime = new AbortController(), ownedSignal = AbortSignal.any([signal, lifetime.signal])
  let relay
  function close() { lifetime.abort(); relay?.close() }
  try {
    relay = components.installRelay({ httpServer: server.httpServer, worldOrigin: settings.worldOrigin,
      frontendOrigin: settings.origin, eventId: settings.eventId, principalIds: config.members.map(member => member.userId),
      createFaultController: components.createFaultController, decodeServer: text => protocol.decode(text, 'server'),
      ...(enableConsumedTicket ? { decodeClient: text => protocol.decode(text, 'client') } : {}), guard, signal: ownedSignal })
    const probeConsumedTicket = enableConsumedTicket ? components.createTicketProbe({ binding, config, relay,
      ownerControl: components.ownerControl, worldOrigin: settings.worldOrigin,
      decodeServer: text => protocol.decode(text, 'server'), guard, signal: ownedSignal }) : undefined
    const checkpoint = components.createCheckpoints({ binding, config, principalIds: config.members.map(member => member.userId),
      relay, ownerControl: components.ownerControl, probeConsumedTicket, guard, signal: ownedSignal })
    return Object.freeze({ relay, checkpoint, binding: Object.freeze({ ...binding }), guard, signal: ownedSignal, close })
  } catch (error) { close(); throw error }
}
