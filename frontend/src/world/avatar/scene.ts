/** Minimal capability for the existing Phaser scene; no Phaser types, engine construction or RAF. */
export interface AvatarSceneSprite {
  update(x: number, y: number, frame: number, depth: number): void
  dispose(): void
}
export interface AvatarSceneTexture { readonly key: string; dispose(): void }
export interface AvatarScenePort {
  texture(canvas: HTMLCanvasElement): AvatarSceneTexture
  /** One genuine 64×64 front-idle cell, with frame 0 only. Separate from layered sheets. */
  staticTexture64?(canvas: HTMLCanvasElement): AvatarSceneTexture
  sprite(texture: AvatarSceneTexture): AvatarSceneSprite
  onFrame(listener: (time: number) => void): () => void
  read(): Readonly<{ textures: number; sprites: number; listeners: number }>
}
