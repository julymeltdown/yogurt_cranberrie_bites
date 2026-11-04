import { useId } from 'react'
import contours from './reference-contours.json'

/** Isolated non-character foreground contours transcribed from the unchanged
 * user-selected reference. No raster, whole-screen UI, or target character is embedded.
 * Source pin and reproducible authoring recipe accompany the path data. */
function ReferenceLayer({ name, className }: { name: 'wordmark' | 'web' | 'scene'; className: string }) {
  const layer = contours.layers.find(candidate => candidate.name === name)!
  return <svg className={className} viewBox="10 35 286 485" preserveAspectRatio="none" aria-hidden="true" focusable="false" data-reference-layer={name}>
    <g fillRule="evenodd">{layer.paths.map((path, index) => <path key={index} fill={path.fill} d={path.d} />)}</g>
  </svg>
}

export const PosterWordmark = () => <ReferenceLayer name="wordmark" className="landing-wordmark" />
export const PosterWeb = () => <ReferenceLayer name="web" className="landing-web" />
export const PosterScene = () => <ReferenceLayer name="scene" className="landing-scene" />

/** Symmetric distressed frame and home-link stars, not counterfeit controls. */
export function PosterWear() {
  const layer = contours.layers.find(candidate => candidate.name === 'frame')!
  return <svg className="landing-print-wear" viewBox="6 33 296 494" preserveAspectRatio="none" aria-hidden="true" focusable="false" data-reference-layer="frame">
    <g fillRule="evenodd">{layer.paths.map((path, index) => <path key={index} fill={path.fill} d={path.d} />)}</g>
  </svg>
}

export function PosterStars() {
  const layer = contours.layers.find(candidate => candidate.name === 'stars')!
  return <svg className="landing-window-stars" viewBox="16 13 53 16" aria-hidden="true" focusable="false">
    <g fillRule="evenodd">{layer.paths.map((path, index) => <path key={index} fill={path.fill} d={path.d} />)}</g>
  </svg>
}

/** Printed copy remains paired with real semantic text in Landing. */
export function PosterCopyPrint() {
  const layer = contours.layers.find(candidate => candidate.name === 'copy')!
  return <svg className="landing-copy-print" viewBox="65 391 185 67" preserveAspectRatio="none" aria-hidden="true" focusable="false">
    <g fillRule="evenodd">{layer.paths.map((path, index) => <path key={index} fill={path.fill} d={path.d} />)}</g>
  </svg>
}

/** Foreground lettering/distress inside the genuine canonical sign-in Link. */
export function PosterEntryPrint() {
  const layer = contours.layers.find(candidate => candidate.name === 'entry')!
  return <svg className="landing-entry-print" viewBox="38 467 236 39" preserveAspectRatio="none" aria-hidden="true" focusable="false">
    <g fillRule="evenodd">{layer.paths.map((path, index) => <path key={index} fill={path.fill} d={path.d} />)}</g>
  </svg>
}

/** Fine monochrome paper noise; not a tiled dot grid or target screenshot. */
export function PosterGrain() {
  const id = useId()
  return <svg className="landing-paper-grain" aria-hidden="true" focusable="false">
    <defs><filter id={id} x="0" y="0" width="100%" height="100%">
      <feTurbulence type="fractalNoise" baseFrequency=".78" numOctaves="3" seed="17" />
      <feColorMatrix type="saturate" values="0" />
      <feComponentTransfer><feFuncA type="linear" slope=".045" /></feComponentTransfer>
    </filter></defs>
    <rect width="100%" height="100%" filter={`url(#${id})`} />
  </svg>
}
