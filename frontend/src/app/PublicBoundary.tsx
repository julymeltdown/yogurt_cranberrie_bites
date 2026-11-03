import { Link, useLocation, useParams } from 'react-router-dom'
import { FoundationApp } from './FoundationApp'
import { DocumentPage } from '../features/public/ui/DocumentPage'
import { SignIn } from '../features/auth/ui/SignIn'
import { ResetPassword } from '../features/auth/ui/ResetPassword'
import { Callback } from '../features/auth/ui/Callback'

/** Lazy public-only boundary. No game/network imports and no fabricated session. */
export default function PublicBoundary() {
  const location = useLocation(), params = useParams()
  if (location.pathname === '/') return <FoundationApp />
  if (location.pathname === '/login') return <SignIn />
  if (location.pathname === '/reset-password') return <ResetPassword />
  if (location.pathname === '/auth/callback') return <Callback />
  if (location.pathname.startsWith('/legal/')) return <DocumentPage document={params.document} />
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
    <div className="shell-actions"><Link className="shell-link" to="/">Back to home</Link></div>
  </main>
}
