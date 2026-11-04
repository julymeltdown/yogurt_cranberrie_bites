import assert from 'node:assert/strict'
import type { Page } from '@playwright/test'
import type { Snapshot } from '../real/mural-helpers.ts'
import type { QualifiedBinding } from './runtime.ts'
import { qualifiedPhaserModuleUrl } from './runtime.ts'

/** Read-only DEV observation adapted from the production-owner default-entry
 * preview.ts:137-149 at canonical 2ff0. No fetch/socket replacement, globals,
 * mock provenance, mutable scene access, or two-sample shortcut is carried over.
 */
export async function verifyWorldTexture(page: Page, snapshot: Snapshot, binding: QualifiedBinding) {
  assert(Date.parse(binding.expiresAt) > Date.now())
  const moduleUrl = qualifiedPhaserModuleUrl(binding.phaserModuleUrl, new URL(page.url()).origin)
  const observed = await page.evaluate(async ({ moduleUrl, key }) => {
    const Phaser = (await import(moduleUrl) as { default: typeof import('phaser') }).default
    const pool = Reflect.get(Phaser.Display.Canvas.CanvasPool, 'pool') as { parent: unknown }[]
    const games = [...new Set(pool.map(item => item.parent as import('phaser').Game).filter(game => game?.canvas?.closest?.('.shell-world-host')))]
    if (games.length !== 1 || document.querySelectorAll('.shell-world-host canvas').length !== 1) throw Error('EXACTLY_ONE_ACTUAL_PHASER_GAME_REQUIRED')
    const scene = games[0]!.scene.getScene('mcr-map')
    if (!scene.textures.exists(key)) throw Error('QUALIFIED_WALL_TEXTURE_REQUIRED')
    const images = scene.children.list.filter(child => child instanceof Phaser.GameObjects.Image && child.texture.key === key) as import('phaser').GameObjects.Image[]
    if (images.length !== 1) throw Error('EXACTLY_ONE_WALL_IMAGE_REQUIRED')
    const canvas = scene.textures.get(key).getSourceImage()
    if (!(canvas instanceof HTMLCanvasElement) || canvas.width !== 128 || canvas.height !== 128) throw Error('ACTUAL_128_SQUARE_CANVAS_TEXTURE_REQUIRED')
    const image = images[0]!
    return { x: image.x, y: image.y, width: image.displayWidth, height: image.displayHeight, depth: image.depth,
      rgba: Array.from(canvas.getContext('2d')!.getImageData(0, 0, 128, 128).data) }
  }, { moduleUrl, key: binding.configuration.textureKey })
  const { rgba, ...anchor } = observed
  assert.deepEqual(anchor, binding.configuration.anchor)
  const expected = snapshot.pixels.flatMap(color => {
    const rgb = snapshot.palette[color]!
    return [parseInt(rgb.slice(1, 3), 16), parseInt(rgb.slice(3, 5), 16), parseInt(rgb.slice(5, 7), 16), 255]
  })
  assert.deepEqual(rgba, expected)
}
