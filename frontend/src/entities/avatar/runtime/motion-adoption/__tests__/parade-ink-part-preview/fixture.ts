import { readFile } from 'node:fs/promises'
import { URL as FileURL } from 'node:url'
import { vi } from 'vitest'
import { validateMotionArt, type MotionArt } from '../../art'
import { createMotionRuntime, type MotionAuthority } from '../../runtime'
import { testMotionAuthority, type TestMotionConfig } from '../../../../../../world/avatar/__tests__/motion-adoption/test-authority'

export async function artwork() {
  return validateMotionArt(new Uint8Array(await readFile(new FileURL('../../../../../../../public/world/avatars/derived-v1/motion-candidates-v1/combined-release-candidate.json', import.meta.url))))
}
export function fixture(art: MotionArt, authority: MotionAuthority<TestMotionConfig> = testMotionAuthority(art)) {
  const drawImage = vi.fn(), canvases: HTMLCanvasElement[] = [], cleanups = new Set<() => void>()
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(function (this: HTMLCanvasElement) {
    if (!canvases.includes(this)) canvases.push(this)
    return { drawImage, clearRect: vi.fn(), save: vi.fn(), restore: vi.fn(), beginPath: vi.fn(), rect: vi.fn(), clip: vi.fn(), fillRect: vi.fn() } as unknown as CanvasRenderingContext2D
  })
  let alive = true
  const imageDispose = vi.fn(), image = new Image(), load = vi.fn(async () => ({ art, image, dispose: imageDispose }))
  const runtime = createMotionRuntime({ authority, scope: { userId: 'self', eventId: 'event', generation: 1, policyGeneration: 1, gate: 'open' },
    privacy: { isCurrent: () => alive, registerCleanup(fn) { cleanups.add(fn); return () => { cleanups.delete(fn) } } }, origin: 'https://mcr.test', load })
  return { runtime, authority, load, imageDispose, image, drawImage, canvases,
    ready: () => runtime.catalog(new AbortController().signal),
    revoke() { alive = false; for (const fn of Array.from(cleanups)) fn() } }
}
export const signal = () => new AbortController().signal
export const part = { kind: 'hair' as const, key: 'hair-0' }
