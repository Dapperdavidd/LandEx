import { useEffect, useState } from 'react'
import { apiRequest } from '../lib/api'
import type { PropertyPage } from '../types/property'

type PropertyState =
  | { status: 'loading'; data: null; message: null }
  | { status: 'ready'; data: PropertyPage; message: null }
  | { status: 'error'; data: null; message: string }

export function useProperties(parameters: URLSearchParams): PropertyState {
  const query = parameters.toString()
  const [result, setResult] = useState<{ query: string; state: PropertyState }>({
    query,
    state: { status: 'loading', data: null, message: null },
  })
  useEffect(() => {
    const controller = new AbortController()
    apiRequest<PropertyPage>(`/properties?${query}`, { signal: controller.signal })
      .then((data) => setResult({ query, state: { status: 'ready', data, message: null } }))
      .catch((error: unknown) => {
        if (error instanceof DOMException && error.name === 'AbortError') return
        setResult({ query, state: { status: 'error', data: null, message: error instanceof Error ? error.message : 'Property data is unavailable.' } })
      })
    return () => controller.abort()
  }, [query])
  return result.query === query ? result.state : { status: 'loading', data: null, message: null }
}
