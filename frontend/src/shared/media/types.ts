import type { SessionScope } from '../api/http'
export type ImageMime = 'image/jpeg' | 'image/png' | 'image/webp'
export type MediaPurpose = 'POST' | 'GIVEAWAY' | 'REPORT'
export interface ReadyMedia {
  readonly media_id: string; readonly state: 'READY'; readonly revision: number
  readonly width: number; readonly height: number; readonly content_type: ImageMime
  readonly purpose: MediaPurpose; readonly read_path: string
}
export interface PreparedImage {
  readonly blob: Blob; readonly width: number; readonly height: number; readonly sha256: string
}
export type UploadPhase = 'idle' | 'preparing' | 'prepared' | 'initializing' | 'uploading' | 'finalizing' | 'ready' | 'failed' | 'uncertain' | 'cancelled'
export interface UploadState {
  readonly phase: UploadPhase
  readonly scope: SessionScope | null
  readonly previewUrl: string | null
  /** Percent in the current stage; null means the transport cannot measure it. */
  readonly progress: number | null
  readonly attachment: ReadyMedia | null
  /** Safe local error code only; no raw exception, body, URL, token or filename. */
  readonly error: string | null
}
export interface ImageUploadPort {
  read(): UploadState
  subscribe(listener: () => void): () => void
  select(file: File): Promise<void>
  upload(): Promise<void>
  recover(): Promise<void>
  /** Local preparation retry only. Uncertain server mutations are never replayed. */
  retry(): Promise<void>
  cancel(): void
  dispose(): void
}
