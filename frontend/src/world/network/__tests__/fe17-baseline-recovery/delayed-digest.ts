import { afterEach, beforeEach, vi } from 'vitest'

const digest = crypto.subtle.digest.bind(crypto.subtle)
beforeEach(() => {
  vi.spyOn(crypto.subtle, 'digest').mockImplementation(async (...args) => {
    // Preserve real SHA-256 and the production ticket flow; vary completion time only.
    await new Promise(resolve => setTimeout(resolve, 75))
    return digest(...args)
  })
})
afterEach(() => vi.restoreAllMocks())
