// Read-only observation of the incumbent game. No renderer, camera, input or clock creation.
import Phaser from 'phaser'
let initialGame,initialCamera
export function observe(){
 const pool=Reflect.get(Phaser.Display.Canvas.CanvasPool,'pool')
 const game=pool.map(item=>item.parent).find(value=>value?.canvas?.closest?.('.shell-world-host'))
 if(!game)throw Error('Missing incumbent game')
 const scene=game.scene.getScene('mcr-map'),camera=scene.cameras.main
 initialGame??=game;initialCamera??=camera
 return {sameGame:game===initialGame,sameCamera:camera===initialCamera,sceneKey:scene.scene.key,
  camera:{x:camera.scrollX,y:camera.scrollY,width:camera.width,height:camera.height,zoom:camera.zoom},
  textures:game.textures.getTextureKeys(),fixture:window.__PI08_FIXTURE__}
}
