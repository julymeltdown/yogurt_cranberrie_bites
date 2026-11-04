import { afterEach, beforeEach, vi } from 'vitest'
import { webcrypto } from 'node:crypto'

// JSDOM exposes a window Crypto without SubtleCrypto. Use the actual Node24
// WebCrypto implementation there; never replace hashing with a fixture result.
if (!globalThis.crypto?.subtle) Object.defineProperty(globalThis, 'crypto', { value: webcrypto, configurable: true })
const digest = crypto.subtle.digest.bind(crypto.subtle)
beforeEach(context => {
  // The original transport cancellation cases deliberately install their own
  // single-call never-resolving digest spy. Do not reuse that spy or contaminate
  // its call count with this separate admission-latency instrumentation.
  if (context.task.file.filepath.endsWith('/network/__tests__/transport.test.ts')) return
  vi.spyOn(crypto.subtle, 'digest').mockImplementation(async (...args) => {
    // Preserve actual bytes and hashing; exercise real ticket admission after variable latency.
    await new Promise(resolve => setTimeout(resolve, 75))
    return digest(...args)
  })
})
afterEach(() => vi.restoreAllMocks())
