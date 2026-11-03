import type Phaser from 'phaser'
import type { LoadedMap } from '../map/resources'
import { sceneryDepth, validateLoadedScenery } from './model'

/** Hidden texture/object preparation in the incumbent scene; caller owns atomic show/remove. */
export function prepareIllustratedSkin(scene: Phaser.Scene, next: LoadedMap, prefix: string, nearest: Phaser.Textures.FilterMode,
  uploaded: () => void, alive: () => boolean) {
  validateLoadedScenery(next)
  const loaded = next.scenery
  if (!loaded) throw new Error('Illustrated resources unavailable')
  const textures: string[] = [], objects: Phaser.GameObjects.Image[] = []
  let removed = false
  const remove = () => {
    if (removed) return; removed = true
    for (const object of objects) object.destroy()
    for (const key of textures) if (scene.textures.exists(key)) scene.textures.remove(key)
  }
  try {
    const keys = new Map<string, string>()
    for (const { descriptor, image } of loaded.assets) {
      const key = `${prefix}-${descriptor.id}`
      // Register cleanup before upload: Phaser texture events can synchronously reenter teardown.
      textures.push(key)
      const texture = scene.textures.addImage(key, image)
      if (!texture) throw new Error('Illustrated texture unavailable')
      texture.setFilter(nearest); keys.set(descriptor.id, key); uploaded()
      if (!alive()) throw new Error('Illustrated scene disposed')
    }
    objects.push(scene.add.image(0, 0, keys.get(loaded.manifest.ground.id)!).setOrigin(0).setDepth(0).setVisible(false))
    for (const o of loaded.manifest.objects) {
      const key = keys.get(o.atlas_id)!, f = o.frame
      const texture = scene.textures.get(key)
      if (!texture.add(o.id, 0, f.x, f.y, f.width, f.height)) throw new Error('Illustrated frame unavailable')
      objects.push(scene.add.image(o.x, o.y, key, o.id).setOrigin(0).setDisplaySize(o.width, o.height)
        .setDepth(sceneryDepth(o.depth_y, next.heightPx)).setVisible(false))
    }
    return { show() { if (removed || !alive()) throw new Error('Illustrated skin disposed'); for (const object of objects) object.setVisible(true) }, remove }
  } catch (error) { remove(); throw error }
}
