// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { waitFor } from '@testing-library/react'
import selected from '../../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json'
import { createAccountOwner } from '../../account-entry/owner'
import { createWholeCharacterRuntime, validateWholeCharacterConfig } from '../../../entities/avatar/whole-character'
import { participant, setup } from '../../../features/me/__tests__/fixtures'
import { bootstrapFor } from '../../__tests__/fixtures/ports'

const stops: (() => void)[] = []
afterEach(() => { stops.splice(0).reverse().forEach(stop => stop()); vi.restoreAllMocks() })
function fixture() {
  const original = participant()
  let server = { ...original, profile: { ...original.profile, avatar: { revision: 1, config: validateWholeCharacterConfig(selected.presets[0]!.config) } } }
  const puts: unknown[] = [], fetcher = vi.fn<typeof fetch>(async input => {
    const request = input as Request
    if (request.method === 'PUT') {
      const body = await request.json(); puts.push(body)
      expect(body.expected_revision).toBe(server.profile.avatar.revision)
      server = { ...server, revision: server.revision + 1, profile: { ...server.profile,
        avatar: { revision: server.profile.avatar.revision + 1, config: validateWholeCharacterConfig(body.config) } } }
      return Response.json(server.profile.avatar)
    }
    return Response.json(server)
  })
  const f = setup(fetcher), scope = f.auth.privacy.capture()
  const runtime = createWholeCharacterRuntime({ privacy: f.auth.privacy, origin: 'https://mcr.test', manifest: selected, pin: selected })
  const choices = selected.presets.map((p, i) => {
    const config = validateWholeCharacterConfig(p.config)
    return { id: config.preset_id, label: `Character ${i + 1}`, config }
  })
  vi.spyOn(runtime, 'choices').mockResolvedValue(choices)
  const bootstrap = { ...bootstrapFor({ ...scope, userId: scope.userId!, eventId: scope.eventId! }), document: { avatar_manifest: selected } }
  const owner = createAccountOwner({ auth: f.auth, scope, bootstrap, characters: runtime })
  stops.push(() => { owner.dispose(); runtime.dispose(); f.dispose() })
  return { ...f, scope, runtime, bootstrap, owner, puts, fetcher, choices, server: () => server }
}
it('borrows one current runtime for the actual whole editor and refreshes Me only after verified save', async () => {
  const f = fixture(), editor = f.owner.wholeAvatar!
  expect(editor.runtime).toBe(f.runtime); expect(editor.read().phase).toBe('idle')
  await f.owner.me.load(); editor.start(); await waitFor(() => expect(editor.read().phase).toBe('ready'))
  expect(editor.select(f.choices[4]!.id)).toBe(true)
  expect(f.owner.me.read().participant?.profile.avatar.config).toEqual(f.choices[0]!.config)
  expect(f.puts).toHaveLength(0)
  expect(await editor.save()).toBe(true)
  await waitFor(() => expect(f.owner.me.read().participant?.profile.avatar.revision).toBe(2))
  expect(f.owner.me.read().participant?.profile.avatar.config).toEqual(f.choices[4]!.config)
  expect(f.puts).toEqual([{ expected_revision: 1, config: f.choices[4]!.config }])
  expect(f.fetcher.mock.calls.every(([request]) => new URL((request as Request).url).pathname.includes('/me'))).toBe(true)
  editor.stop(); expect(f.runtime.isCurrent()).toBe(true)
  f.owner.dispose(); expect(f.runtime.isCurrent()).toBe(true)
})
it('keeps a dirty text profile draft when a character save requests an authoritative refresh', async () => {
  const f = fixture(), editor = f.owner.wholeAvatar!
  await f.owner.me.load(); f.owner.me.editProfile({ bio: 'Keep my biography' }); editor.start()
  await waitFor(() => expect(editor.read().phase).toBe('ready'))
  editor.select(f.choices[1]!.id); await editor.save()
  expect(f.owner.me.read().draft.bio).toBe('Keep my biography'); expect(f.owner.me.read().blocked).toBe(true)
  expect(f.server().profile.avatar.revision).toBe(2)
})
it('releases the editor on privacy invalidation and never revives the borrowed old scope', async () => {
  const f = fixture(), editor = f.owner.wholeAvatar!
  editor.start(); await waitFor(() => expect(editor.read().phase).toBe('ready'))
  f.auth.privacy.changePolicy(f.scope, 'consent-revoked')
  expect(f.owner.isDisposed()).toBe(true); expect(editor.read().phase).toBe('disposed'); expect(f.runtime.isCurrent()).toBe(false)
  expect(editor.select(f.choices[2]!.id)).toBe(false); expect(await editor.save()).toBe(false); expect(f.puts).toHaveLength(0)
})
it.each(['userId', 'eventId', 'generation', 'policyGeneration', 'gate'] as const)('rejects a borrowed runtime with a different %s', field => {
  const f = fixture(), scope = { ...f.runtime.scope, [field]: field.endsWith('Generation') || field === 'generation' ? 99 : 'mismatched' }
  expect(() => createAccountOwner({ auth: f.auth, scope: f.scope, bootstrap: f.bootstrap,
    characters: { ...f.runtime, scope } as typeof f.runtime })).toThrow('unavailable')
})
