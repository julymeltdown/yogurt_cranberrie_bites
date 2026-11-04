/** Original print assets are supplied by ART-WINDOW-CONTROLS-02.
 * The containing native control owns its name, focus and pointer target.
 */
export function WindowInk({ name, className = '' }: { name: 'close' | 'back' | 'expand' | 'reduce' | 'stars'; className?: string }) {
  return <img className={`mcr-reference-window-ink${name === 'stars' ? ' mcr-reference-window-ink--stars' : ''} ${className}`} src={`/reference-window-controls-v2/${name}.svg`}
    alt="" aria-hidden="true" draggable={false} width={name === 'stars' ? 72 : 24} height={24} />
}
