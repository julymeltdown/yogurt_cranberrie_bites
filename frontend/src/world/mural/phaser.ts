import type Phaser from 'phaser'
import type { MuralScenePort, MuralSceneTexture } from './sink'
/** Bind to an existing, admitted Phaser.Scene and its reviewed virtual wall anchor. Creates no game. */
export function createPhaserMuralScene(scene: Phaser.Scene, anchor: { readonly x: number; readonly y: number; readonly width: number; readonly height: number; readonly depth: number }, key = 'mcr-mural'): MuralScenePort {
  if (![anchor.x, anchor.y, anchor.width, anchor.height, anchor.depth].every(Number.isFinite) || anchor.width <= 0 || anchor.height <= 0
    || anchor.width > 2048 || anchor.height > 2048 || !/^[a-zA-Z0-9_-]{1,80}$/.test(key)) throw new Error('Invalid mural anchor')
  const textures = new Map<MuralSceneTexture, string>()
  let closed = false
  return {
    texture() {
      if (closed || textures.size || scene.textures.exists(key)) throw new Error('Mural texture already owned')
      const texture = scene.textures.createCanvas(key, 128, 128)
      if (!texture) throw new Error('Mural texture unavailable')
      texture.setFilter(1) // Phaser.Textures.FilterMode.NEAREST, without evaluating Phaser at module load.
      let disposed = false
      const lease: MuralSceneTexture = {
        replace(rgba) {
          if (closed || disposed || rgba.length !== 65536) throw new Error('Mural texture unavailable')
          const context = texture.getContext(), image = context.createImageData(128, 128); image.data.set(rgba)
          context.putImageData(image, 0, 0); texture.refresh()
        },
        dispose() { if (!disposed) { disposed = true; textures.delete(lease); scene.textures.remove(key) } },
      }
      textures.set(lease, key); return lease
    },
    object(texture) {
      const owned = textures.get(texture)
      if (closed || !owned) throw new Error('Unowned mural texture')
      const image = scene.add.image(anchor.x, anchor.y, owned).setOrigin(0, 0).setDisplaySize(anchor.width, anchor.height).setDepth(anchor.depth)
      let disposed = false
      return { dispose() { if (!disposed) { disposed = true; image.destroy() } } }
    },
    onFrame(listener) { if (closed) throw new Error('Mural scene closed'); scene.events.on('update', listener); return () => { scene.events.off('update', listener) } },
    onClose(listener) {
      const close = () => { closed = true; listener() }
      scene.events.once('shutdown', close); scene.events.once('destroy', close)
      return () => { scene.events.off('shutdown', close); scene.events.off('destroy', close) }
    },
  }
}
