export const themeIds = ['REVENGE', 'PARADE', 'DANGER'] as const
export type ThemeId = typeof themeIds[number]
export const isThemeId = (value: unknown): value is ThemeId => themeIds.some(id => id === value)
export interface ThemeScope { readonly userId: string; readonly eventId: string }
/** Synchronous local preference boundary; never represents server persistence. */
export interface ThemePreferences {
  read(scope: ThemeScope): unknown
  write(scope: ThemeScope, theme: ThemeId): void
}
export function createLocalThemePreferences(storage: Pick<Storage, 'getItem' | 'setItem'>): ThemePreferences {
  const key = (scope: ThemeScope) => {
    if (!scope.userId || !scope.eventId) throw new Error('Theme preference requires an identity and event')
    return `mcr:theme:v1:${encodeURIComponent(scope.userId)}:${encodeURIComponent(scope.eventId)}`
  }
  return { read: scope => storage.getItem(key(scope)), write: (scope, theme) => storage.setItem(key(scope), theme) }
}
