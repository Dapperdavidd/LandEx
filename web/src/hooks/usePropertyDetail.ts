import { useEffect, useState } from 'react'
import { apiRequest } from '../lib/api'
import type { PropertyHistoryPoint, PropertyListItem, PropertyLocationIntelligence, PropertyScore } from '../types/property'

export interface PropertyDetailData { property: PropertyListItem; history: PropertyHistoryPoint[]; score: PropertyScore; location: PropertyLocationIntelligence }
type DetailState = { status: 'loading' } | { status: 'ready'; data: PropertyDetailData } | { status: 'error'; message: string }

export function usePropertyDetail(id: string | undefined): DetailState {
  const [result, setResult] = useState<{ id: string | undefined; state: DetailState }>({ id, state: { status: 'loading' } })
  useEffect(() => {
    if (!id) return
    const controller = new AbortController()
    Promise.all([
      apiRequest<PropertyListItem>(`/properties/${id}`, { signal: controller.signal }),
      apiRequest<PropertyHistoryPoint[]>(`/properties/${id}/history?limit=120`, { signal: controller.signal }),
      apiRequest<PropertyScore>(`/properties/${id}/score`, { signal: controller.signal }),
      apiRequest<PropertyLocationIntelligence>(`/properties/${id}/location-intelligence?radius_meters=3000&limit=20`, { signal: controller.signal }),
    ]).then(([property, history, score, location]) => setResult({ id, state: { status: 'ready', data: { property, history, score, location } } }))
      .catch((error: unknown) => {
        if (error instanceof DOMException && error.name === 'AbortError') return
        setResult({ id, state: { status: 'error', message: error instanceof Error ? error.message : 'Property intelligence is unavailable.' } })
      })
    return () => controller.abort()
  }, [id])
  if (!id) return { status: 'error', message: 'The property identifier is missing.' }
  return result.id === id ? result.state : { status: 'loading' }
}
