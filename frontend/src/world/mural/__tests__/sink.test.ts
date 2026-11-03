import { describe, expect, it, vi } from 'vitest'
import { createMuralTextureSink } from '../sink'
import type { MuralFrame } from '../sink'
import { authorityPort, memoryScene, snapshot } from '../../../features/mural/sync/__tests__/fixture'
const frame = (revision = 10): MuralFrame => ({ binding: authorityPort().read()!.binding, revision, state: 'EDITABLE', palette: snapshot().palette, pixels: snapshot().pixels })
describe('FE33 incumbent-scene mural resource sink', () => {
  it('coalesces many updates to one frame and one texture/object; rejects revision rewind', () => {
    const scene = memoryScene(), sink = createMuralTextureSink(scene.port)
    for (let i = 10; i < 600; i++) sink.replace(frame(i))
    expect(sink.read()).toMatchObject({ queued: 1, textures: 0, listeners: 1 }); scene.frame()
    expect(scene.read()).toMatchObject({ textures: 1, objects: 1, uploads: 1, listeners: 1 }); expect(sink.read().revision).toBe(599)
    sink.replace(frame(10)); expect(sink.read().queued).toBe(0); sink.dispose(); sink.dispose()
    expect(scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0 })
  })
  it('copies caller arrays and clears resources immediately on withdrawal', () => {
    const scene = memoryScene(), sink = createMuralTextureSink(scene.port), source = frame()
    sink.replace(source); (source.pixels as number[])[0] = 2; scene.frame()
    expect(scene.read().rgba![0]).toBe(250); sink.replace(null); expect(scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0 })
  })
  it('epoch change disposes the old scene leases before replacing them', () => {
    const scene = memoryScene(), sink = createMuralTextureSink(scene.port); sink.replace(frame()); scene.frame()
    sink.replace({ ...frame(20), binding: { ...frame().binding, streamId: '00000000-0000-4000-8000-000000000999' } })
    expect(scene.read()).toMatchObject({ textures: 0, objects: 0 }); scene.frame(); expect(scene.read()).toMatchObject({ textures: 1, objects: 1 }); sink.dispose()
  })
  it('allocation failure releases partially created texture and notifies failure without a frame-loop exception', () => {
    const scene = memoryScene(), failure = vi.fn(), sink = createMuralTextureSink({ ...scene.port, object() { throw new Error('scene ended') } })
    sink.subscribeFailure!(failure); sink.replace(frame()); expect(() => scene.frame()).not.toThrow()
    expect(failure).toHaveBeenCalledOnce(); expect(scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0 })
  })
  it('invalid raster clears prior resources before throwing to its owner', () => {
    const scene = memoryScene(), sink = createMuralTextureSink(scene.port); sink.replace(frame()); scene.frame()
    expect(() => sink.replace({ ...frame(11), pixels: [0] })).toThrow(); expect(scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0 })
  })
  it('incumbent scene shutdown withdraws texture/object/frame work and notifies the model', () => {
    const scene = memoryScene(), failure = vi.fn(); let close = () => {}, registered = false
    const sink = createMuralTextureSink({ ...scene.port, onClose(listener) { close = listener; registered = true; return () => { registered = false } } })
    sink.subscribeFailure!(failure); sink.replace(frame()); scene.frame(); close()
    expect(registered).toBe(false); expect(failure).toHaveBeenCalledOnce(); expect(scene.read()).toMatchObject({ textures: 0, objects: 0, listeners: 0 })
  })
})
