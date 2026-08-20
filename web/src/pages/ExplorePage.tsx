import { useMemo, useState, type FormEvent } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useAuth } from '../auth/auth-context'
import { DataLabel } from '../components/DataLabel'
import { useProperties } from '../hooks/useProperties'
import { apiRequest } from '../lib/api'
import { formatMoney, formatPercent } from '../lib/format'
import type { PropertyListItem, SavedSearch } from '../types/property'

const fields = ['country_code', 'currency', 'property_type', 'listing_type', 'min_price', 'max_price', 'min_yield_percent', 'min_growth_percent', 'min_score'] as const

export function ExplorePage() {
  const auth = useAuth()
  const [searchParams, setSearchParams] = useSearchParams({ limit: '20' })
  const query = useMemo(() => new URLSearchParams(searchParams), [searchParams])
  const properties = useProperties(query)
  const [saveOpen, setSaveOpen] = useState(false)
  const [searchName, setSearchName] = useState('')
  const [saveMessage, setSaveMessage] = useState<string | null>(null)

  function applyFilters(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const form = new FormData(event.currentTarget)
    const next = new URLSearchParams({ limit: '20' })
    fields.forEach((field) => {
      const value = String(form.get(field) ?? '').trim()
      if (value) next.set(field, value)
    })
    setSearchParams(next)
  }

  async function saveSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!auth.accessToken) return
    const criteria = Object.fromEntries([...searchParams.entries()].filter(([key]) => key !== 'limit' && key !== 'offset'))
    try {
      await apiRequest<SavedSearch>('/saved-searches', {
        method: 'POST',
        headers: { Authorization: `Bearer ${auth.accessToken}` },
        body: JSON.stringify({ name: searchName, criteria }),
      })
      setSaveMessage('Search saved. New-match alerts can now track this market view.')
      setSearchName('')
      setSaveOpen(false)
    } catch (error) {
      setSaveMessage(error instanceof Error ? error.message : 'Search could not be saved.')
    }
  }

  return <main className="explore-page">
    <header className="explore-header"><div><DataLabel>02 / Opportunity research</DataLabel><h1>Find an<br />opportunity.</h1></div><div className="explore-header__aside"><span>{properties.status === 'ready' ? properties.data.total : '—'}</span><DataLabel>Matching active listings</DataLabel></div></header>
    <form className="filter-terminal" onSubmit={applyFilters}>
      <label><span>Country</span><input name="country_code" maxLength={2} defaultValue={searchParams.get('country_code') ?? ''} placeholder="GLOBAL" /></label>
      <label><span>Currency</span><input name="currency" maxLength={3} defaultValue={searchParams.get('currency') ?? ''} placeholder="ANY" /></label>
      <label><span>Asset</span><select name="property_type" defaultValue={searchParams.get('property_type') ?? ''}><option value="">All types</option><option value="apartment">Apartment</option><option value="house">House</option><option value="commercial">Commercial</option><option value="land">Land</option><option value="hotel">Hotel</option><option value="retail">Retail</option><option value="industrial">Industrial</option></select></label>
      <label><span>Listing</span><select name="listing_type" defaultValue={searchParams.get('listing_type') ?? ''}><option value="">All listings</option><option value="sale">Sale</option><option value="rent">Rent</option><option value="shortlet">Shortlet</option></select></label>
      <label><span>Min price</span><input name="min_price" inputMode="decimal" defaultValue={searchParams.get('min_price') ?? ''} placeholder="0" /></label>
      <label><span>Max price</span><input name="max_price" inputMode="decimal" defaultValue={searchParams.get('max_price') ?? ''} placeholder="NO LIMIT" /></label>
      <label><span>Yield ≥</span><input name="min_yield_percent" inputMode="decimal" defaultValue={searchParams.get('min_yield_percent') ?? ''} placeholder="—" /></label>
      <label><span>Growth ≥</span><input name="min_growth_percent" inputMode="decimal" defaultValue={searchParams.get('min_growth_percent') ?? ''} placeholder="—" /></label>
      <label><span>Score ≥</span><input name="min_score" inputMode="decimal" defaultValue={searchParams.get('min_score') ?? ''} placeholder="—" /></label>
      <button type="submit">Run search <span>↗</span></button>
    </form>
    <div className="explore-actions"><DataLabel>Results / latest normalized observations</DataLabel><div>{auth.status === 'authenticated' ? <button type="button" onClick={() => { setSaveOpen((value) => !value); setSaveMessage(null) }}>Save this search</button> : <Link to="/access" state={{ from: `/explore?${searchParams}` }}>Sign in to save</Link>}<button type="button" onClick={() => setSearchParams({ limit: '20' })}>Clear filters</button></div></div>
    {saveOpen && <form className="save-search" onSubmit={saveSearch}><label><span>Search name</span><input required maxLength={100} value={searchName} onChange={(event) => setSearchName(event.target.value)} placeholder="e.g. Lagos income watch" /></label><button type="submit">Save</button></form>}
    {saveMessage && <p className="save-message" role="status">{saveMessage}</p>}
    {properties.status === 'loading' && <ExploreState>Scanning normalized inventory…</ExploreState>}
    {properties.status === 'error' && <ExploreState error>{properties.message}</ExploreState>}
    {properties.status === 'ready' && properties.data.items.length === 0 && <ExploreState>No active properties match this market view.</ExploreState>}
    {properties.status === 'ready' && properties.data.items.length > 0 && <PropertyResults properties={properties.data.items} />}
  </main>
}

function PropertyResults({ properties }: { properties: PropertyListItem[] }) {
  return <section className="property-results" aria-label="Property results"><div className="property-results__head"><span>Asset</span><span>Price</span><span>Yield</span><span>Growth</span><span>Score</span></div>{properties.map((property, index) => <Link className="property-result" to={`/properties/${property.id}`} key={property.listing_id}><div className="property-result__identity"><span>{String(index + 1).padStart(2, '0')}</span><div><strong>{property.address_line || `${property.property_type} / ${property.location_name}`}</strong><small>{property.location_name}, {property.country_code} · {property.listing_type} · {property.bedrooms ?? '—'} BED</small></div></div><span>{formatMoney(property.price, property.currency)}{property.price_period !== 'total' && <small>/{property.price_period}</small>}</span><span>{formatPercent(property.gross_yield_percent)}</span><span className={property.annual_growth_percent === null ? '' : Number(property.annual_growth_percent) >= 0 ? 'signal--positive' : 'signal--negative'}>{formatPercent(property.annual_growth_percent)}</span><span>{property.overall_score ?? '—'}</span></Link>)}</section>
}
function ExploreState({ children, error = false }: { children: string; error?: boolean }) { return <div className={`explore-state${error ? ' explore-state--error' : ''}`}>{children}</div> }
