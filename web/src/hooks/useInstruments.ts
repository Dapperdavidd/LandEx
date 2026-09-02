import { useEffect, useState } from 'react'
import { apiRequest } from '../lib/api'
import type { InstrumentPage } from '../types/instrument'

type State = { status: 'loading'; data: null; message: null } | { status: 'ready'; data: InstrumentPage; message: null } | { status: 'error'; data: null; message: string }

export function useInstruments(countryCode = '', limit = 100, providerSlug = ''): State {
  const [state, setState] = useState<State>({ status: 'loading', data: null, message: null })
  useEffect(() => {
    const controller = new AbortController()
    const query = new URLSearchParams({ limit: String(limit) })
    if (countryCode) query.set('country_code', countryCode)
    if (providerSlug) query.set('provider_slug', providerSlug)
    apiRequest<InstrumentPage>(`/instruments?${query}`, { signal: controller.signal })
      .then((data) => setState({ status: 'ready', data, message: null }))
      .catch((error: unknown) => {
        if (error instanceof DOMException && error.name === 'AbortError') return
        setState({ status: 'error', data: null, message: error instanceof Error ? error.message : 'Instrument data is unavailable.' })
      })
    return () => controller.abort()
  }, [countryCode, limit, providerSlug])
  return state
}
