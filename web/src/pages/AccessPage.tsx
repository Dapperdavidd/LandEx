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
      <p className="access-disclosure">Paper investing is simulated. No real property is purchased through this account.</p>
    </section>
  </main>
}
