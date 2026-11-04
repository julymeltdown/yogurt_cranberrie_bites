// Private, one-shot custody of the FIRST actual browser authenticate frame.
// A real validated server snapshot must confirm consumption before release.
import assert from 'node:assert/strict'
import { TextDecoder } from 'node:util'

export function createAuthenticationCapture({ decodeClient, eventId, guard }) {
  assert.equal(typeof decodeClient, 'function'); assert.equal(typeof guard, 'function')
  let pending = Buffer.alloc(0), authentication = null, principal = null, captured = false, disposed = false
  function check() {
    assert(!disposed, 'QA05_CAPTURE_RETIRED')
    const result = guard(); assert(result !== false && !result?.then)
  }
  function dispose() { disposed = true; authentication = null; principal = null; pending.fill(0); pending = Buffer.alloc(0) }
  return Object.freeze({
    feed(bytes) {
      check()
      if (captured) return
      try {
        assert(bytes instanceof Uint8Array && pending.length + bytes.length <= 262144)
        pending = Buffer.concat([pending, bytes])
        if (pending.length < 2) return
        assert(pending[0] === 0x81 && (pending[1] & 0x80), 'QA05_AUTHENTICATE_FRAME_REQUIRED')
        const code = pending[1] & 127, header = code < 126 ? 2 : code === 126 ? 4 : 10
        if (pending.length < header) return
        const size = code < 126 ? code : code === 126 ? pending.readUInt16BE(2) : Number(pending.readBigUInt64BE(2))
        assert(Number.isSafeInteger(size) && size > 0 && size <= 65536)
        assert(code < 126 || size >= (code === 126 ? 126 : 65536))
        if (pending.length < header + 4 + size) return
        const mask = pending.subarray(header, header + 4), payload = Buffer.alloc(size)
        for (let n = 0; n < size; n++) payload[n] = pending[header + 4 + n] ^ mask[n % 4]
        try {
          const value = decodeClient(new TextDecoder('utf-8', { fatal: true }).decode(payload))
          assert(value.type === 'authenticate' && value.eventId === eventId && typeof value.ticket === 'string')
          // Retain exact decoded values without normalizing an actual ticket.
          authentication = structuredClone(value); captured = true
        } finally { payload.fill(0); pending.fill(0); pending = Buffer.alloc(0) }
      } catch {
        dispose()
        throw new Error('QA05_AUTHENTICATE_CAPTURE_REFUSED')
      }
    },
    consumed(snapshot) {
      check()
      if (principal !== null) {
        assert(snapshot.type === 'snapshot' && snapshot.eventId === eventId && snapshot.selfUserId === principal)
        return
      }
      assert(captured && authentication && snapshot.type === 'snapshot' && snapshot.eventId === eventId)
      for (const key of ['roomId', 'mapId', 'mapRevision', 'avatarManifestId', 'avatarManifestVersion']) {
        assert.equal(snapshot[key], authentication[key], 'QA05_CONSUMED_CONTEXT_MISMATCH')
      }
      assert(typeof snapshot.selfUserId === 'string' && snapshot.selfUserId.length > 0)
      assert(principal === null || principal === snapshot.selfUserId)
      principal = snapshot.selfUserId
    },
    take(principalId) {
      check()
      assert(principal !== null && principalId === principal && authentication, 'QA05_CONSUMED_AUTHENTICATION_REQUIRED')
      const value = authentication; authentication = null
      return value
    },
    dispose,
  })
}
