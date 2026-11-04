import type { ComponentType } from 'react'
import type { SessionController } from '../../features/auth/model/session'
import type { PrivacyScope, PolicyChange } from '../../shared/query/privacy'
import type { ShellBootstrap } from '../ports'
import type { ImageUploadPort } from '../../features/feed-thread'
import type { MediaPreviewPort } from '../../features/event-feed'
import type { AvatarResources } from '../../features/profile/Avatar'
export const services = ['feed', 'people', 'intro', 'age', 'inbox', 'requests'] as const
export type SocialService = typeof services[number]
/** Readiness/authorization projection, not an added C03 wire capability. */
export interface SocialAvailability {
  readonly scope: PrivacyScope
  readonly bootstrap: ShellBootstrap
  readonly services: Readonly<Partial<Record<SocialService, boolean>>>
}
export interface SocialAvailabilityPort { read(): SocialAvailability | null; subscribe(listener: () => void): () => void }
export interface SocialContext { readonly auth: SessionController; readonly bootstrap: ShellBootstrap; readonly scope: PrivacyScope }
export interface SocialDestination { readonly path: string; readonly scope: PrivacyScope }
export interface RequestsModule {
  read(): { readonly locked: boolean }
  subscribe(listener: () => void): () => void
  start(): void
  /** Existing shell destinations only; detail stays inside the requests panel. */
  enter?(path: '/me/social' | '/me/requests'): void
  /** Bound to a freshly authorized introduction, never a raw user/URL target. */
  offer?(target: { readonly introId: string; readonly recipientId: string; readonly revision: number }): void
  dispose(): void
  readonly Panel: ComponentType
}
export interface SocialOptions extends SocialContext {
  readonly availability: SocialAvailabilityPort
  readonly navigate: (destination: SocialDestination) => void
  /** Absent FE31 module stays unavailable; no inferred sharing targets or request bodies. */
  readonly requests?: { create(context: SocialContext & { readonly access: SocialAccessPort }): RequestsModule }
  readonly media?: { create(context: SocialContext): { readonly upload: ImageUploadPort; createPreview(mediaId: string): MediaPreviewPort; dispose(): void } }
  readonly avatars?: { resolve(context: SocialContext & { readonly signal: AbortSignal }): Promise<AvatarResources> }
  /** Trusted reviewed provider configuration only. No default verifier/callback route. */
  readonly verification?: { allowOrigin(origin: string): boolean }
}
export interface SocialAccessPort { read(): { readonly available: boolean; readonly readOnly: boolean }; subscribe(listener: () => void): () => void }
export interface SocialPolicyNotification { readonly scope: PrivacyScope; readonly reason: PolicyChange }
