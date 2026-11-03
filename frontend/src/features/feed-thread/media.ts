import type { SessionScope } from '../../shared/api/http'
/** Structural consumer seam for FE04; the shell injects the qualified canonical-session controller. */
export interface ImageUploadPort {
  read(): {
    readonly phase: 'idle' | 'preparing' | 'prepared' | 'initializing' | 'uploading' | 'finalizing' | 'ready' | 'failed' | 'uncertain' | 'cancelled'
    readonly scope: SessionScope | null; readonly previewUrl: string | null; readonly progress: number | null
    readonly attachment: { readonly media_id: string; readonly state: 'READY'; readonly revision: number; readonly width: number; readonly height: number; readonly content_type: 'image/jpeg' | 'image/png' | 'image/webp'; readonly purpose: 'POST' | 'GIVEAWAY' | 'REPORT'; readonly read_path: string } | null
    readonly error: string | null
  }
  subscribe(listener: () => void): () => void
  select(file: File): Promise<void>; upload(): Promise<void>; recover(): Promise<void>; retry(): Promise<void>
  cancel(): void; dispose(): void
}
