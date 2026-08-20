import { createContext, useContext } from 'react'
import type { User } from '../types/auth'

export interface RegisterInput { displayName: string; email: string; password: string }
export interface AuthContextValue {
  status: 'loading' | 'authenticated' | 'guest'
  user: User | null
  accessToken: string | null
  login(email: string, password: string): Promise<void>
  register(input: RegisterInput): Promise<void>
  logout(): Promise<void>
}
export const AuthContext = createContext<AuthContextValue | null>(null)
export function useAuth(): AuthContextValue {
  const context = useContext(AuthContext)
  if (!context) throw new Error('useAuth must be used within AuthProvider')
  return context
}
