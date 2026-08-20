import { useAuth } from '../auth/auth-context'
import { DataLabel } from '../components/DataLabel'
export function ProfilePage() {
  const auth = useAuth(); const user = auth.user!
  return <main className="profile-page"><DataLabel>LandEX / Investor identity</DataLabel><div className="profile-page__header"><h1>{user.display_name}</h1><span>{user.email_verified_at ? 'Email verified' : 'Verification pending'}</span></div><dl className="identity-ledger"><div><dt>Primary email</dt><dd>{user.primary_email}</dd></div><div><dt>Investor since</dt><dd>{new Intl.DateTimeFormat('en', { dateStyle: 'long' }).format(new Date(user.created_at))}</dd></div><div><dt>Identity</dt><dd>{user.id}</dd></div></dl><button className="text-action" type="button" onClick={() => void auth.logout()}>End session</button></main>
}
