// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { readFile } from 'node:fs/promises'
import { URL as FileURL } from 'node:url'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeAll, beforeEach, expect, it, vi } from 'vitest'
import { MemoryRouter } from 'react-router-dom'
import { ReviewedCharacterView } from '../../../../../app/character-entry/ReviewedCharacterView'
import type { ReviewedCharacterOwner } from '../../../../../app/character-entry/reviewed-registry'
import { createMotionRuntime } from '../../../../../entities/avatar/runtime/motion-adoption/runtime'
import { MOTION_ART } from '../../../../../entities/avatar/runtime/motion-adoption/art'
import { GiftDetail, GiftDiscovery, type GiftImageRequest } from '../../../../gift-discovery/ui/Gifts'
import { fixture as gifts, giveaway } from '../../../../gift-discovery/__tests__/fixtures/data'
import { PartyDetail } from '../../../../party-discovery/ui/PartyDiscovery'
import { harness as parties } from '../../../../party-discovery/__tests__/fixtures/harness'
import { ids, party } from '../../../../party-discovery/__tests__/fixtures/data'
import { preset, setup } from '../../reviewed-production/fixtures'

const stops: (() => void)[] = [], resources = new Map<string, Uint8Array<ArrayBuffer>>()
beforeAll(async () => {
  for (const resource of [MOTION_ART.descriptor, MOTION_ART.atlas]) resources.set(resource.path,
    new Uint8Array(await readFile(new FileURL(`../../../../../../public/${resource.path}`, import.meta.url))))
})
beforeEach(() => {
  vi.stubGlobal('Image', class { src = ''; async decode() {} })
  vi.stubGlobal('URL', class extends URL { static createObjectURL() { return 'blob:shared-portrait-test' } static revokeObjectURL() {} })
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ drawImage: vi.fn(), clearRect: vi.fn(), fillRect: vi.fn(),
    save: vi.fn(), restore: vi.fn(), beginPath: vi.fn(), rect: vi.fn(), clip: vi.fn() } as unknown as CanvasRenderingContext2D)
})
afterEach(async () => { cleanup(); await act(async () => stops.splice(0).reverse().forEach(stop => stop())); vi.restoreAllMocks(); vi.unstubAllGlobals() })

async function admitted() {
  const f = await setup(); stops.push(f.dispose)
  const authority = await f.create(); stops.push(authority.dispose)
  const runtime = createMotionRuntime({ authority, scope: f.scope, privacy: f.auth.privacy, origin: location.origin,
    fetch: async input => {
      const url = String(input), bytes = resources.get(new URL(url).pathname.slice(1))
      if (!bytes) throw Error('Unapproved artwork resource')
      const response = new Response(bytes, { headers: { 'Content-Type': url.endsWith('.png') ? 'image/png' : 'application/json', 'Content-Length': String(bytes.length) } })
      Object.defineProperty(response, 'url', { value: url }); return response
    },
  }); stops.push(runtime.dispose)
  const owner: ReviewedCharacterOwner = { kind: 'reviewed-motion', scope: f.scope, authority, runtime,
    isCurrent: () => authority.isCurrent() && runtime.isCurrent(), dispose() { runtime.dispose(); authority.dispose() } }
  return { ...f, owner, runtime }
}
const avatar = () => ({ revision: 1, config: preset(0) })

it('shares one admitted composition across Giveaway discovery/detail and releases only the departing consumer', async () => {
  const f = await admitted(), gift = giveaway({ owner: { ...giveaway().owner, avatar: avatar() } })
  const g = gifts({ handler: request => Response.json(new URL(request.url).pathname.endsWith('/giveaways') ? { items: [gift], next_cursor: null } : gift) }); stops.push(g.dispose)
  const portrait = (owner: typeof gift.owner) => <ReviewedCharacterView owner={f.owner} config={owner.avatar.config} label={`${owner.display_name}'s avatar`} size={64} />
  const list = render(<MemoryRouter><GiftDiscovery controller={g.controller} renderOwnerPortrait={portrait} /></MemoryRouter>)
  const detail = render(<MemoryRouter><GiftDetail controller={g.controller} giveawayId={gift.giveaway_id} renderOwnerPortrait={portrait} /></MemoryRouter>)
  await waitFor(() => expect(document.querySelectorAll('[data-motion-state="ready"]')).toHaveLength(2))
  expect(f.runtime.diagnostics().composition).toMatchObject({ entries: 1, references: 2 })
  expect(screen.getByRole('link', { name: `View public place reference` })).toBeVisible()
  expect(screen.getByRole('button', { name: 'Reserve unavailable' })).toBeDisabled()
  list.unmount()
  expect(f.runtime.diagnostics().leases).toBe(1); expect(f.owner.isCurrent()).toBe(true)
  expect(detail.container.querySelector('[data-motion-state="ready"]')).not.toBeNull()
  expect(g.requests.every(request => request.method === 'GET')).toBe(true)
})

it('preserves the protected Gift photo request and withdraws the portrait on privacy invalidation', async () => {
  const f = await admitted(), gift = giveaway({ image_media_id: '00000000-0000-4000-8000-000000000099', owner: { ...giveaway().owner, avatar: avatar() } })
  const g = gifts({ handler: () => Response.json(gift) }); stops.push(g.dispose)
  const photo = vi.fn((_request: GiftImageRequest) => <span>Protected photo consumer</span>)
  render(<MemoryRouter><GiftDetail controller={g.controller} giveawayId={gift.giveaway_id} renderGiftImage={photo}
    renderOwnerPortrait={owner => <ReviewedCharacterView owner={f.owner} config={owner.avatar.config} label="Gift host" size={64} />} /></MemoryRouter>)
  await screen.findByRole('img', { name: 'Gift host' })
  await waitFor(() => expect(document.querySelector('[data-motion-state="ready"]')).not.toBeNull())
  expect(photo.mock.calls[0]?.[0]).toMatchObject({ mediaId: gift.image_media_id, giveawayId: gift.giveaway_id, scope: g.context()!.scope })
  await act(async () => { f.auth.privacy.changePolicy(f.scope, 'consent-revoked'); g.auth.privacy.changePolicy(g.context()!.scope, 'consent-revoked') })
  expect(document.querySelector('[data-motion-state="ready"]')).toBeNull()
  expect(screen.queryByText('Protected photo consumer')).not.toBeInTheDocument()
  expect(f.runtime.diagnostics().leases).toBe(0)
})

it('renders the existing Party owner without inventing member portraits or changing application controls', async () => {
  const f = await admitted(), p = party({ owner: { ...party().owner, avatar: avatar() } })
  const h = parties(request => Response.json(new URL(request.url).pathname.endsWith('/parties') ? { items: [p], next_cursor: null } : p)); stops.push(h.dispose)
  render(<MemoryRouter><PartyDetail controller={h.controller} partyId={ids.party}
    renderOwnerPortrait={owner => <ReviewedCharacterView owner={f.owner} config={(owner.avatar as ReturnType<typeof avatar>).config} label="Party host" size={64} />} /></MemoryRouter>)
  await waitFor(() => expect(document.querySelector('[data-motion-state="ready"]')).not.toBeNull())
  expect(screen.getByRole('button', { name: 'Apply to party' })).toBeVisible()
  expect(screen.getByLabelText(/Note to the host/)).toHaveValue('')
  expect(screen.getAllByRole('img')).toHaveLength(1)
  expect(h.requests.every(request => request.method === 'GET')).toBe(true)
})

it.each([null, { ...preset(0), manifest_id: 'foreign' }, { ...preset(0), manifest_version: 99 }, { ...preset(0), extra: 'untrusted' }])(
  'rejects malformed or foreign owner config without borrowing artwork (%j)', async config => {
    const f = await admitted(), acquire = vi.spyOn(f.runtime, 'acquire')
    render(<ReviewedCharacterView owner={f.owner} config={config} label="Untrusted owner" size={64} />)
    expect(screen.getByText('Character unavailable')).toBeVisible()
    expect(acquire).not.toHaveBeenCalled(); expect(f.runtime.diagnostics().leases).toBe(0)
  },
)

it('rejects a stale owner without a legacy or preset fallback', async () => {
  const f = await admitted(), acquire = vi.spyOn(f.runtime, 'acquire')
  f.owner.dispose()
  render(<ReviewedCharacterView owner={f.owner} config={preset(0)} label="Stale owner" size={64} />)
  expect(screen.getByText('Character unavailable')).toBeVisible()
  expect(acquire).not.toHaveBeenCalled(); expect(screen.queryByRole('img', { name: 'Stale owner' })).not.toBeInTheDocument()
})
