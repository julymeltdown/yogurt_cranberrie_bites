import assert from 'node:assert/strict'
import { ownedBytes } from './private-file.mjs'
export { ownedBytes } from './private-file.mjs'

export function exactKeys(value, names) {
  assert(value && typeof value === 'object' && !Array.isArray(value))
  assert.deepEqual(Object.keys(value).sort(), [...names].sort(), 'Unexpected input fields')
}

export function loopbackOrigin(value) {
  const url = new URL(value)
  assert.equal(url.origin, value, 'Use an exact origin without credentials/path/query/hash')
  assert.equal(url.protocol, 'http:')
  assert(['127.0.0.1', '[::1]', 'localhost'].includes(url.hostname), 'Only the existing loopback runtime is supported')
  assert(url.port && Number(url.port) > 1023, 'Explicit nonprivileged port required')
  return url
}

export async function productionValidators(source) {
  // These two canonical TS modules have no runtime imports. Node's built-in
  // stripping keeps their exact validation implementation; no parallel schema.
  const { stripTypeScriptTypes } = await import('node:module')
  const load = async path => import('data:text/javascript;base64,' + Buffer.from(stripTypeScriptTypes(source.bytes(path).toString(), { mode: 'strip' })).toString('base64'))
  const [mural, input] = await Promise.all([
    load('frontend/src/app/world-entry/mural-production/configuration.ts'),
    load('frontend/src/app/world-entry/configuration.ts'),
  ])
  return { mural: mural.validateMuralProductionConfiguration, localInput: input.validateLocalInputConfiguration }
}

export async function readSettings(path, source) {
  const settings = JSON.parse((await ownedBytes(path, { privateOnly: true })).toString())
  exactKeys(settings, ['version', 'candidate', 'environmentId', 'eventId', 'origin', 'gatewayOrigin', 'worldOrigin', 'localInput', 'mural'])
  assert.equal(settings.version, 1)
  assert.equal(settings.candidate, source.revision)
  for (const key of ['environmentId', 'eventId']) assert.match(settings[key], /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/)
  const origin = loopbackOrigin(settings.origin)
  for (const key of ['gatewayOrigin', 'worldOrigin']) {
    loopbackOrigin(settings[key])
    assert.notEqual(settings[key], origin.origin, 'Proxy cannot target itself')
  }
  const validators = await productionValidators(source)
  assert(validators.localInput(settings.localInput), 'Actual qualified local-input configuration is required')
  assert(validators.mural(settings.mural), 'Actual map-bound production mural placement is required')
  assert.equal(settings.mural.anchor.width, 128); assert.equal(settings.mural.anchor.height, 128)
  return settings
}
