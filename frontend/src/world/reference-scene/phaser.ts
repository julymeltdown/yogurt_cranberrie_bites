import type Phaser from 'phaser'
import type { LoadedMap } from '../map/resources'
import { sceneryDepth } from '../illustrated/model'
import { requireReferenceScene } from './model'
import { preparePhoto2SceneSkin } from './photo2-world-display-v1/phaser'

/** Hidden preparation on the incumbent scene. No physics, actor, input, camera or network ownership. */
export function prepareReferenceSceneSkin(scene: Phaser.Scene, map: LoadedMap, prefix: string,
  nearest: Phaser.Textures.FilterMode, uploaded: () => void, alive: () => boolean) {
  const binding = requireReferenceScene(map)
  if (binding.display) return preparePhoto2SceneSkin(scene, map, binding.display, prefix, nearest, uploaded, alive)
  const objects: Phaser.GameObjects.Image[] = [], keys: string[] = []
  let removed = false, graphics: Phaser.GameObjects.Graphics | null = null, mask: Phaser.Display.Masks.GeometryMask | null = null
  const remove = () => {
    if (removed) return; removed = true
    // Continue cleanup even if one injected/failed resource throws.
    for (const object of objects) { try { object.destroy() } catch { /* Independent owned objects. */ } }
    try { mask?.destroy() } catch { /* Continue releasing texture storage. */ }
    try { graphics?.destroy() } catch { /* Continue releasing texture storage. */ }
    for (const key of keys) { try { if (scene.textures.exists(key)) scene.textures.remove(key) } catch { /* Independent owned textures. */ } }
  }
  const check = () => { if (removed || !alive()) throw new Error('Reference scene unavailable') }
  try {
    check()
    const textures = new Map<string, string>()
    for (const asset of binding.loaded.assets) {
      const key = `${prefix}-${asset.descriptor.id}`
      keys.push(key)
      const texture = scene.textures.addImage(key, asset.image)
      if (!texture) throw new Error('Reference texture unavailable')
      texture.setFilter(nearest); textures.set(asset.descriptor.id, key); uploaded(); check()
    }
    // Alpha is preserved by the source crop. This mask only clips illustration overhang to the one world.
    graphics = scene.make.graphics({ x: 0, y: 0 }, false)
    graphics.fillStyle(0xffffff).fillRect(0, 0, map.widthPx, map.heightPx)
    mask = graphics.createGeometryMask(); check()
    function image(x: number, y: number, key: string, frame?: string) {
      check()
      const object = scene.add.image(x, y, key, frame); objects.push(object)
      object.setOrigin(0).setVisible(false).setMask(mask!); check(); return object
    }
    image(0, 0, textures.get(binding.loaded.manifest.ground.id)!).setDepth(0)
    for (const prop of binding.loaded.manifest.objects) {
      const key = textures.get(prop.atlas_id)!, f = prop.frame
      if (!scene.textures.get(key).add(prop.id, 0, f.x, f.y, f.width, f.height)) throw new Error('Reference frame unavailable')
      image(prop.x, prop.y, key, prop.id).setDisplaySize(prop.width, prop.height)
        .setDepth(sceneryDepth(prop.depth_y, map.heightPx))
    }
    check()
    return { show() {
      try { check(); for (const object of objects) { object.setVisible(true); check() } }
      catch (error) { remove(); throw error }
    }, remove }
  } catch (error) { remove(); throw error }
}
