import { useEffect, useState } from 'react'
import { apiRequest } from '../lib/api'
import type { MarketPage } from '../types/market'

type MarketState =
  | { status: 'loading'; data: null; message: null }
  | { status: 'ready'; data: MarketPage; message: null }
  | { status: 'error'; data: null; message: string }

export function useMarkets(): MarketState {
  const [state, setState] = useState<MarketState>({ status: 'loading', data: null, message: null })

  useEffect(() => {
    const controller = new AbortController()
    apiRequest<MarketPage>('/markets?limit=8', { signal: controller.signal })
      .then((data) => setState({ status: 'ready', data, message: null }))
      .catch((error: unknown) => {
        if (error instanceof DOMException && error.name === 'AbortError') return
        setState({
          status: 'error',
          data: null,
          message: error instanceof Error ? error.message : 'Market data is unavailable.',
        })
      })
    return () => controller.abort()
  }, [])

  return state
}
