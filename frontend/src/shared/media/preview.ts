export interface PreviewPort { create(blob: Blob): string; revoke(url: string): void }
export const browserPreviews: PreviewPort = {
  create: blob => URL.createObjectURL(blob), revoke: url => URL.revokeObjectURL(url),
}
