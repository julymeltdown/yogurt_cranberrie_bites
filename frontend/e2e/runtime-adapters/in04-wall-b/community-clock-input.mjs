// Root-owned compiled input custody; no measurement or authority is generated.
import assert from 'node:assert/strict'
import { exactKeys, ownedBytes } from './inputs.mjs'
import { sha256 } from './pinned-source.mjs'

export const clockInputModule = '/src/app/community-clock-runtime/input.ts'
export const clockInputName = 'import.meta.env.VITE_MCR_COMMUNITY_CLOCK_INPUT'
const bindingKeys = ['candidate', 'environmentId', 'eventId', 'origin', 'bootstrapContractSha256', 'concertBinarySha256', 'serverClockSource']

export function compiledClockIdentity(input) {
  assert(typeof input.raw === 'string', 'Clock input must retain its compiled string')
  const bytes = Buffer.from(input.raw, 'utf8')
  assert(bytes.length > 0 && bytes.length <= 512 * 1024, 'Clock input size bound')
  return Object.freeze({ name: clockInputName, encoding: 'utf-8', bytes: bytes.length, sha256: sha256(bytes) })
}

export async function readCommunityClockInput(descriptor, { candidate, settings, source }) {
  if (descriptor === undefined) return null
  exactKeys(descriptor, ['path', 'sha256', 'current'])
  exactKeys(descriptor.current, bindingKeys)
  const current = Object.freeze({ ...descriptor.current })
  for (const key of ['candidate', 'environmentId', 'eventId', 'origin']) {
    assert.equal(current[key], key === 'candidate' ? candidate : settings[key], 'Independent current clock binding differs: ' + key)
  }
  for (const key of ['bootstrapContractSha256', 'concertBinarySha256']) assert.match(current[key], /^[a-f0-9]{64}$/)
  assert.match(current.serverClockSource, /^[a-f0-9]{40}$/)
  assert.match(descriptor.sha256, /^[a-f0-9]{64}$/)
  assert(source.files.has('frontend' + clockInputModule), 'Candidate lacks the canonical clock input decoder')
  const bytes = await ownedBytes(descriptor.path, { privateOnly: true, maximum: 512 * 1024, hash: descriptor.sha256 })
  const raw = bytes.toString('utf8')
  assert(Buffer.from(raw, 'utf8').equals(bytes), 'Clock input must be exact UTF-8 bytes')
  return Object.freeze({ raw, current })
}

export function validateCommunityClockInput(input, decode) {
  assert.equal(typeof decode, 'function', 'Canonical clock input decoder is required')
  const decoded = decode(input.raw)
  assert.deepEqual(decoded.current, input.current, 'Captured clock input differs from independently supplied current binding')
  return decoded
}
