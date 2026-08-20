import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react'
import { apiRequest } from '../lib/api'
import type { AuthResponse, SessionTokens, User } from '../types/auth'
import { AuthContext, type AuthContextValue, type RegisterInput } from './auth-context'

const REFRESH_TOKEN_KEY = 'landex.session.refresh'

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null)
  const [accessToken, setAccessToken] = useState<string | null>(null)
  const [status, setStatus] = useState<AuthContextValue['status']>(() =>
    sessionStorage.getItem(REFRESH_TOKEN_KEY) ? 'loading' : 'guest',
  )
  const acceptSession = useCallback((nextUser: User, session: SessionTokens) => {
    sessionStorage.setItem(REFRESH_TOKEN_KEY, session.refresh_token)
    setAccessToken(session.access_token)
    setUser(nextUser)
    setStatus('authenticated')
  }, [])
  const clearSession = useCallback(() => {
    sessionStorage.removeItem(REFRESH_TOKEN_KEY)
    setAccessToken(null)
    setUser(null)
    setStatus('guest')
  }, [])
  useEffect(() => {
    const refreshToken = sessionStorage.getItem(REFRESH_TOKEN_KEY)
    if (!refreshToken) return
    apiRequest<SessionTokens>('/auth/refresh', { method: 'POST', body: JSON.stringify({ refresh_token: refreshToken }) })
      .then(async (session) => {
        const nextUser = await apiRequest<User>('/auth/me', { headers: { Authorization: `Bearer ${session.access_token}` } })
        acceptSession(nextUser, session)
      })
      .catch(clearSession)
  }, [acceptSession, clearSession])
  const login = useCallback(async (email: string, password: string) => {
    const response = await apiRequest<AuthResponse>('/auth/login', { method: 'POST', body: JSON.stringify({ email, password }) })
    acceptSession(response.user, response.session)
  }, [acceptSession])
  const register = useCallback(async ({ displayName, email, password }: RegisterInput) => {
    const response = await apiRequest<AuthResponse>('/auth/register', { method: 'POST', body: JSON.stringify({ display_name: displayName, email, password }) })
    acceptSession(response.user, response.session)
  }, [acceptSession])
  const logout = useCallback(async () => {
    const token = accessToken
    clearSession()
    if (token) await apiRequest<void>('/auth/logout', { method: 'POST', headers: { Authorization: `Bearer ${token}` } }).catch(() => undefined)
  }, [accessToken, clearSession])
  const value = useMemo<AuthContextValue>(() => ({ status, user, accessToken, login, register, logout }), [status, user, accessToken, login, register, logout])
  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>
}
