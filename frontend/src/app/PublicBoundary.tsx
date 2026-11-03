import { Link, useLocation } from 'react-router-dom'
import { FoundationApp } from './FoundationApp'
import { safeReturnTarget } from './routes'

/** Lazy public-only boundary. No game/network imports and no fabricated session. */
export default function PublicBoundary() {
  const location = useLocation()
  if (location.pathname === '/') return <FoundationApp />
  const returnTo = safeReturnTarget(new URLSearchParams(location.search).get('returnTo'))
  const title = location.pathname === '/login' ? 'Sign in'
    : location.pathname === '/signup' ? 'Create account'
      : location.pathname === '/verify-email' ? 'Verify email'
        : location.pathname === '/reset-password' ? 'Reset password'
          : location.pathname === '/auth/callback' ? 'Connecting'
            : location.pathname === '/account/deleted' ? 'Account status'
              : location.pathname.startsWith('/legal/') ? 'Notices'
                : location.pathname === '/archive' ? 'After the show' : 'Page unavailable'
  return <main className="shell-gate" id="main" tabIndex={-1}>
    <Link className="shell-link" to="/">MCR Concert World</Link><h1>{title}</h1>
    <p>{title === 'Page unavailable' ? 'This page could not be found.'
      : 'This page is still being prepared. No account action has been completed.'}</p>
    {location.pathname === '/login' && <p>Your destination will remain available after sign-in is connected.</p>}
    <div className="shell-actions"><Link className="shell-link" to="/">Back to home</Link>
      {location.pathname === '/login' && <Link className="shell-link" to={returnTo}>Back to your destination</Link>}</div>
  </main>
}
