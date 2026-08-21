import { useState, type FormEvent } from 'react'
import { Navigate, useLocation, useNavigate } from 'react-router-dom'
import { useAuth } from '../auth/auth-context'
import { DataLabel } from '../components/DataLabel'

type Mode = 'login' | 'register'
export function AccessPage() {
  const auth = useAuth(); const navigate = useNavigate(); const location = useLocation()
  const [mode, setMode] = useState<Mode>('login'); const [displayName, setDisplayName] = useState('')
  const [email, setEmail] = useState(''); const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null); const [submitting, setSubmitting] = useState(false)
  if (auth.status === 'authenticated') return <Navigate to="/" replace />
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setError(null); setSubmitting(true)
    try {
      if (mode === 'login') await auth.login(email, password); else await auth.register({ displayName, email, password })
      navigate((location.state as { from?: string } | null)?.from ?? '/', { replace: true })
    } catch (reason) { setError(reason instanceof Error ? reason.message : 'Access could not be established.') }
    finally { setSubmitting(false) }
  }
  return <main className="access-page">
    <section className="access-page__statement"><DataLabel>LandEX / Private terminal</DataLabel><h1>Enter the<br />market.</h1><p>Build a point of view. Test it with capital. Learn from every position.</p></section>
    <section className="access-panel" aria-labelledby="access-title">
      <div className="access-tabs"><button className={mode === 'login' ? 'is-active' : ''} type="button" onClick={() => setMode('login')}>Sign in</button><button className={mode === 'register' ? 'is-active' : ''} type="button" onClick={() => setMode('register')}>Create account</button></div>
      <div className="access-panel__heading"><DataLabel>{mode === 'login' ? 'Existing investor' : 'New investor'}</DataLabel><h2 id="access-title">{mode === 'login' ? 'Resume your position.' : 'Start with $100K demo.'}</h2></div>
      <form onSubmit={submit}>
        {mode === 'register' && <label><span>Display name</span><input required maxLength={100} autoComplete="name" value={displayName} onChange={(event) => setDisplayName(event.target.value)} /></label>}
        <label><span>Email</span><input required type="email" autoComplete="email" value={email} onChange={(event) => setEmail(event.target.value)} /></label>
        <label><span>Password</span><input required type="password" minLength={12} autoComplete={mode === 'login' ? 'current-password' : 'new-password'} value={password} onChange={(event) => setPassword(event.target.value)} /></label>
        {mode === 'register' && <p className="field-note">Use at least 12 characters. Google and Apple access will follow later.</p>}
        {error && <p className="form-error" role="alert">{error}</p>}
        <button className="primary-action" disabled={submitting || auth.status === 'loading'} type="submit">{submitting ? 'Establishing session…' : mode === 'login' ? 'Enter terminal' : 'Open demo account'}</button>
      </form>
      <div className="access-divider"><span>Or continue with</span></div>
      <div className="social-access" aria-label="Additional sign-in methods coming soon">
        <button type="button" disabled title="Google sign-in is coming soon">
          <svg aria-hidden="true" viewBox="0 0 24 24"><path fill="currentColor" d="M21.6 12.23c0-.71-.06-1.39-.18-2.05H12v3.87h5.38a4.6 4.6 0 0 1-2 3.02v2.51h3.24c1.9-1.75 2.98-4.33 2.98-7.35Z"/><path fill="currentColor" d="M12 22c2.7 0 4.98-.9 6.64-2.42l-3.24-2.51c-.9.6-2.05.96-3.4.96-2.61 0-4.82-1.76-5.61-4.13H3.04v2.59A10 10 0 0 0 12 22Z" opacity=".78"/><path fill="currentColor" d="M6.39 13.9A6.02 6.02 0 0 1 6.08 12c0-.66.11-1.3.31-1.9V7.51H3.04A10 10 0 0 0 2 12c0 1.61.39 3.14 1.04 4.49l3.35-2.59Z" opacity=".58"/><path fill="currentColor" d="M12 5.97c1.47 0 2.79.5 3.83 1.5l2.88-2.88A9.65 9.65 0 0 0 12 2a10 10 0 0 0-8.96 5.51l3.35 2.59C7.18 7.73 9.39 5.97 12 5.97Z" opacity=".9"/></svg>
          <span>Google</span><small>Soon</small>
        </button>
        <button type="button" disabled title="Apple sign-in is coming soon">
          <svg aria-hidden="true" viewBox="0 0 24 24"><path fill="currentColor" d="M17.05 12.54c-.03-3.03 2.48-4.5 2.6-4.57a5.58 5.58 0 0 0-4.4-2.38c-1.85-.2-3.65 1.11-4.59 1.11-.96 0-2.41-1.09-3.98-1.05a5.82 5.82 0 0 0-4.9 2.99c-2.13 3.69-.54 9.12 1.5 12.1 1.02 1.46 2.2 3.08 3.77 3.02 1.53-.06 2.1-.97 3.94-.97 1.82 0 2.36.97 3.96.93 1.64-.02 2.68-1.46 3.66-2.93a12.08 12.08 0 0 0 1.68-3.42 5.25 5.25 0 0 1-3.24-4.83ZM14.04 3.63A5.34 5.34 0 0 0 15.27 0a5.43 5.43 0 0 0-3.51 1.72 5.1 5.1 0 0 0-1.27 3.49 4.49 4.49 0 0 0 3.55-1.58Z"/></svg>
          <span>Apple</span><small>Soon</small>
        </button>
      </div>
      <p className="access-disclosure">Paper investing is simulated. No real property is purchased through this account.</p>
    </section>
  </main>
}
