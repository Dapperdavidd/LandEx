export interface User {
  id: string
  display_name: string
  primary_email: string
  email_verified_at: string | null
  created_at: string
}
export interface SessionTokens {
  access_token: string
  refresh_token: string
  token_type: 'Bearer'
  access_expires_at: string
  refresh_expires_at: string
}
export interface AuthResponse { user: User; session: SessionTokens }
