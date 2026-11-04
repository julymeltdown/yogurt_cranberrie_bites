import { afterEach, expect, it, vi } from 'vitest'
import { createHttpClient } from '../../http'
import { createMuralHttp } from '../../../../features/mural/sync/http'
import { authorityPort, httpClientFixture, ids, serverFixture, timestamp } from '../../../../features/mural/sync/__tests__/fixture'
import type { PixelCommand } from '../../../../features/mural/editor/contract'
afterEach(() => vi.unstubAllGlobals())
it('the existing FE33 adapter treats reason-only cooldown as definitive rejection instead of unknown, with no implicit resend', async () => {
  vi.stubGlobal('location', { origin: 'https://mcr.example' })
  const authority = authorityPort(), fixture = httpClientFixture(serverFixture(), ids.user, authority)
  const sent: { method: string; key: string | null }[] = []
  const http = createHttpClient({ session: fixture.session, fetch: async input => {
    const r = input as Request; sent.push({ method: r.method, key: r.headers.get('Idempotency-Key') })
    return Response.json({ code: 'MURAL_COOLDOWN_REPLAY', message: 'Fixture failed replay.', details: { reason: 'MURAL_COOLDOWN' } }, { status: 429 })
  } })
  const command: PixelCommand = { scope: fixture.privacy.capture(), operation: 'placeConcertPixel', key: ids.next, body: { expected_revision: 10, x: 1, y: 2, color_index: 2 } }
  try {
    const adapter = createMuralHttp({ http, privacy: fixture.privacy, authority, now: () => timestamp })
    expect(await adapter.commands.place(command, new AbortController().signal)).toEqual({ kind: 'rejected', reason: 'MURAL_COOLDOWN' })
    expect(sent).toEqual([{ method: 'POST', key: ids.next }])
  } finally { fixture.dispose() }
})
