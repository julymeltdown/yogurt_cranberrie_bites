/** Borrowed effects on one incumbent engine; no game, transport or audio authority. */
export interface RuntimeEffectLease { dispose(): void }
export type RuntimeEffectValue = Readonly<{ reducedMotion: boolean; lowPower: boolean }>
export interface RuntimeEffects {
  readonly motion: boolean
  readonly lowPower: boolean
  acquire(value: RuntimeEffectValue, signal: AbortSignal): RuntimeEffectLease | Promise<RuntimeEffectLease>
}
