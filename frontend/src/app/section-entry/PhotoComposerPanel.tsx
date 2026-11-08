/**
 * WP-09 T-904: the photo composer screen.
 *
 * `/me/posts/new` publishes an ordinary photograph or artwork post. The composer
 * itself is WP-06's (`features/profile-media`); this panel binds the shell's
 * session to it and gives the screen its heading and its way back.
 *
 * The composer is deliberately independent of the intro composer and of the room:
 * publishing here touches neither, and nothing on this screen offers a poster or
 * a pixel conversion.
 */
import { useId, useMemo } from 'react'
import { useNavigate } from 'react-router-dom'
import { usePorts } from '../access'
import { ShellLink } from '../navigation'
import { ProfileMediaComposer, createProfileMediaComposer } from '../../features/profile-media'
import './section-entry.css'

/** `/me/posts/new` — the photo composer. */
export function PhotoComposerPanel() {
  const ports = usePorts()
  const auth = ports.auth
  const navigate = useNavigate()
  const id = useId()
  const composer = useMemo(() => {
    if (!auth) return null
    const scope = auth.privacy.capture()
    if (!auth.privacy.isCurrent(scope)) return null
    return createProfileMediaComposer({
      auth: { session: auth.session, privacy: auth.privacy, request: auth.request },
      scope,
      // The browser digest: SHA-256 of the picked bytes, lowercase hex.
      digest: async bytes => {
        const buffer = await crypto.subtle.digest('SHA-256', bytes as unknown as ArrayBuffer)
        return Array.from(new Uint8Array(buffer), byte => byte.toString(16).padStart(2, '0')).join('')
      },
      nextLocalId: () => crypto.randomUUID(),
    })
  }, [auth])
  if (!composer) {
    return <section className="section-page" aria-labelledby={`${id}-title`}>
      <header className="section-header"><h1 id={`${id}-title`}>New photo post</h1></header>
      <p role="status">Posting is unavailable until your session is verified.</p>
      <ShellLink to="/me">Back to your profile</ShellLink>
    </section>
  }
  return <section className="section-page" aria-labelledby={`${id}-title`}>
    <header className="section-header"><h1 id={`${id}-title`}>New photo post</h1></header>
    <ProfileMediaComposer composer={composer} onPublished={() => { navigate('/me', { replace: true }) }} />
    <ShellLink to="/me">Back to your profile</ShellLink>
  </section>
}
