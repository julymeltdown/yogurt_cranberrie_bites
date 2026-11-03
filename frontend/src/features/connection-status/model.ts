export type ConnectionKind = 'ready' | 'offline' | 'reconnecting' | 'resync' | 'suspended' | 'unavailable' | 'maintenance' | 'version'
export interface ConnectionStatusView {
  readonly kind: ConnectionKind
  readonly stale: boolean
  readonly pending: boolean
  readonly canRetry: boolean
}
export interface ConnectionStatusController {
  read(): ConnectionStatusView
  subscribe(listener: () => void): () => void
  retry(): Promise<boolean>
}
export const connectionCopy: Record<ConnectionKind, { title: string; description: string }> = {
  ready: { title: 'World is up to date', description: 'Live world information is available.' },
  offline: { title: 'You are offline', description: 'Check your connection. Pickup and other actions are not queued here.' },
  reconnecting: { title: 'Connecting to the world', description: 'Waiting for a fresh world view. You can keep browsing.' },
  resync: { title: 'Updating the world', description: 'Live movement is paused while the world catches up.' },
  suspended: { title: 'World is paused', description: 'The world will recover when its viewport is visible again.' },
  unavailable: { title: 'World connection unavailable', description: 'A fresh world view could not be confirmed. You can keep browsing or reload.' },
  maintenance: { title: 'World is under maintenance', description: 'Live world access is temporarily unavailable. You can return to home.' },
  version: { title: 'Reload to update the world', description: 'This world version cannot be used for live actions. Reload to get the current version.' },
}
