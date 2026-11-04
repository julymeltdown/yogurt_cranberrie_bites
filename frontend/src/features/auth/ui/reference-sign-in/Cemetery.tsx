import topPrint from './artwork/cemetery-top-v1.png'
import bottomPrint from './artwork/cemetery-bottom-v1.png'

/** Authored print layers only; all form text, actions and states remain live DOM. */
export function Cemetery({ edge }: { edge: 'top' | 'bottom' }) {
  return <svg className={`auth-cemetery auth-cemetery--${edge}`} viewBox="0 0 320 120" preserveAspectRatio="none" aria-hidden="true" focusable="false">
    <image href={edge === 'top' ? topPrint : bottomPrint} x="0" y="0" width="320" height="120" preserveAspectRatio="none" />
  </svg>
}
