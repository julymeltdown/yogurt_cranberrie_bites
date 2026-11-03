import type { ThemeId } from '../../features/theme/model/preferences'
import { loadMap } from '../map/resources'

export type ThemeRelease = Parameters<typeof loadMap>[0]
/** Release pins and resource allowlists are composition-owned, never supplied by a selection control. */
export function createThemeLoader(releases: Readonly<Record<ThemeId, ThemeRelease>>) {
  return (theme: ThemeId, signal: AbortSignal) => loadMap(releases[theme], signal)
}
