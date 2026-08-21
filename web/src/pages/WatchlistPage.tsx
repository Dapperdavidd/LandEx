import { useEffect, useState, type FormEvent } from 'react'
import { Link } from 'react-router-dom'
import { useAuth } from '../auth/auth-context'
import { DataLabel } from '../components/DataLabel'
import { apiRequest } from '../lib/api'
import { formatMoney, formatPercent } from '../lib/format'
import type { PropertyListItem, WatchlistDetail, WatchlistSummary } from '../types/property'

interface SavedProperty { itemId: string; property: PropertyListItem }
type WatchlistState = { status: 'loading' } | { status: 'error'; message: string } | { status: 'ready'; lists: WatchlistSummary[]; active: WatchlistDetail | null; properties: SavedProperty[] }

export function WatchlistPage() {
  const auth = useAuth(); const token = auth.accessToken
  const [state, setState] = useState<WatchlistState>({ status: 'loading' }); const [selectedId, setSelectedId] = useState<string | null>(null)
  const [newName, setNewName] = useState(''); const [creating, setCreating] = useState(false); const [message, setMessage] = useState<string | null>(null)
  useEffect(() => {
    if (!token) return
    const controller = new AbortController(); const headers = { Authorization: `Bearer ${token}` }
    apiRequest<WatchlistSummary[]>('/watchlists', { headers, signal: controller.signal }).then(async (lists) => {
      const target = lists.find((list) => list.id === selectedId) ?? lists.at(0) ?? null
      if (!target) { setState({ status: 'ready', lists, active: null, properties: [] }); return }
      const active = await apiRequest<WatchlistDetail>(`/watchlists/${target.id}`, { headers, signal: controller.signal })
      const propertyItems = active.items.filter((item) => item.property_id !== null)
      const properties = await Promise.all(propertyItems.map(async (item) => ({ itemId: item.id, property: await apiRequest<PropertyListItem>(`/properties/${item.property_id}`, { signal: controller.signal }) })))
      setState({ status: 'ready', lists, active, properties })
    }).catch((error: unknown) => {
      if (error instanceof DOMException && error.name === 'AbortError') return
      setState({ status: 'error', message: error instanceof Error ? error.message : 'Watchlists are unavailable.' })
    })
    return () => controller.abort()
  }, [token, selectedId])
  async function createList(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (!token) return; setCreating(true); setMessage(null)
    try { const list = await apiRequest<WatchlistSummary>('/watchlists', { method: 'POST', headers: { Authorization: `Bearer ${token}` }, body: JSON.stringify({ name: newName }) }); setNewName(''); setSelectedId(list.id); setMessage('Watchlist created.') }
    catch (error) { setMessage(error instanceof Error ? error.message : 'Watchlist could not be created.') } finally { setCreating(false) }
  }
  async function remove(itemId: string) {
    if (!token || state.status !== 'ready' || !state.active) return
    try { await apiRequest(`/watchlists/${state.active.id}/items/${itemId}`, { method: 'DELETE', headers: { Authorization: `Bearer ${token}` } }); setState({ ...state, active: { ...state.active, item_count: state.active.item_count - 1, items: state.active.items.filter((item) => item.id !== itemId) }, properties: state.properties.filter((item) => item.itemId !== itemId) }); setMessage('Property removed.') }
    catch (error) { setMessage(error instanceof Error ? error.message : 'Property could not be removed.') }
  }
  return <main className="watchlist-page"><header className="watchlist-header"><div><DataLabel>03 / Signal ledger</DataLabel><h1>Watch<br />the market.</h1></div><p>Hold opportunities in view without committing capital. Every saved asset remains tied to its latest normalized observation.</p></header><section className="watchlist-controls"><div className="watchlist-tabs">{state.status === 'ready' && state.lists.map((list) => <button type="button" className={(state.active?.id ?? selectedId) === list.id ? 'is-active' : ''} onClick={() => setSelectedId(list.id)} key={list.id}>{list.name}<span>{list.item_count}</span></button>)}</div><form onSubmit={createList}><input required maxLength={100} value={newName} onChange={(event) => setNewName(event.target.value)} placeholder="NEW LIST NAME" aria-label="New watchlist name" /><button disabled={creating} type="submit">{creating ? 'Creating…' : 'Create list +'}</button></form></section>{message && <p className="watchlist-message" role="status">{message}</p>}{state.status === 'loading' && <WatchState>Loading saved signals…</WatchState>}{state.status === 'error' && <WatchState error>{state.message}</WatchState>}{state.status === 'ready' && !state.active && <WatchState>Create your first watchlist, then save properties from Explore.</WatchState>}{state.status === 'ready' && state.active && <section className="watchlist-ledger"><div className="watchlist-ledger__head"><span>Asset</span><span>Price</span><span>Yield</span><span>Growth</span><span>Action</span></div>{state.properties.length === 0 ? <WatchState>This watchlist has no saved properties yet.</WatchState> : state.properties.map(({ itemId, property }, index) => <div className="watchlist-entry" key={itemId}><Link to={`/properties/${property.id}`}><span>{String(index + 1).padStart(2, '0')}</span><div><strong>{property.address_line || `${property.property_type} / ${property.location_name}`}</strong><small>{property.location_name}, {property.country_code}</small></div></Link><span>{formatMoney(property.price, property.currency)}</span><span>{formatPercent(property.gross_yield_percent)}</span><span>{formatPercent(property.annual_growth_percent)}</span><button type="button" onClick={() => remove(itemId)}>Remove</button></div>)}</section>}</main>
}
function WatchState({ children, error = false }: { children: string; error?: boolean }) { return <div className={`watchlist-state${error ? ' watchlist-state--error' : ''}`}>{children}</div> }
