import type { ReactNode } from 'react'
import { Navigate, useLocation } from 'react-router-dom'
import { useAuth } from './auth-context'
export function ProtectedRoute({ children }: { children: ReactNode }) {
  const auth = useAuth()
  const location = useLocation()
  if (auth.status === 'loading') return <main className="auth-loading">Recovering your market session…</main>
  if (auth.status === 'guest') return <Navigate to="/access" replace state={{ from: location.pathname }} />
  return children
}
