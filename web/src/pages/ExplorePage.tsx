import { useEffect, useMemo, useState, type FormEvent } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useAuth } from '../auth/auth-context'
import { DataLabel } from '../components/DataLabel'
import { useProperties } from '../hooks/useProperties'
import { apiRequest } from '../lib/api'
import { formatMoney, formatPercent } from '../lib/format'
import type { PropertyListItem, SavedSearch } from '../types/property'

const fields = ['country_code', 'currency', 'property_type', 'listing_type', 'min_price', 'max_price', 'min_yield_percent', 'min_growth_percent', 'min_score'] as const
const choices = { currency: [['USD', 'US dollar'], ['NGN', 'Nigerian naira'], ['GBP', 'British pound'], ['EUR', 'Euro'], ['AED', 'UAE dirham'], ['CAD', 'Canadian dollar'], ['AUD', 'Australian dollar']], property_type: [['apartment', 'Apartment'], ['house', 'House'], ['commercial', 'Commercial'], ['land', 'Land'], ['hotel', 'Hotel'], ['retail', 'Retail'], ['industrial', 'Industrial']], listing_type: [['sale', 'For sale'], ['rent', 'For rent'], ['shortlet', 'Shortlet']], min_yield_percent: [['4', '4%+'], ['6', '6%+'], ['8', '8%+'], ['10', '10%+']], min_growth_percent: [['3', '3%+'], ['5', '5%+'], ['8', '8%+'], ['12', '12%+']], min_score: [['50', '50+'], ['65', '65+'], ['75', '75+'], ['85', '85+']] } as const
type PickerField = keyof typeof choices | 'country_code'
type LocationOption = { id: string; name: string; country_code: string; property_count: number }

export function ExplorePage() {
  const auth = useAuth()
  const [searchParams, setSearchParams] = useSearchParams({ limit: '20' })
  const query = useMemo(() => new URLSearchParams(searchParams), [searchParams])
  const properties = useProperties(query)
  const [saveOpen, setSaveOpen] = useState(false)
  const [searchName, setSearchName] = useState('')
  const [saveMessage, setSaveMessage] = useState<string | null>(null)
  const [picker, setPicker] = useState<PickerField | null>(null)
  const [countries, setCountries] = useState<LocationOption[]>([])
  useEffect(() => { const controller = new AbortController(); apiRequest<LocationOption[]>('/locations?kind=country&limit=100', { signal: controller.signal }).then(setCountries).catch(() => setCountries([])); return () => controller.abort() }, [])
  const labelFor = (field: PickerField) => { const value = searchParams.get(field); if (!value) return field === 'country_code' ? 'Global' : 'Any'; if (field === 'country_code') return countries.find((country) => country.country_code === value)?.name ?? value; return choices[field].find(([key]) => key === value)?.[1] ?? value }

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
      {(['country_code', 'currency', 'property_type', 'listing_type'] as PickerField[]).map((field) => <PickerButton field={field} key={field} label={field === 'country_code' ? 'Country' : field === 'property_type' ? 'Asset' : field === 'listing_type' ? 'Listing' : 'Currency'} value={searchParams.get(field) ?? ''} display={labelFor(field)} onClick={() => setPicker(field)} />)}
      <label><span>Min price</span><input name="min_price" inputMode="decimal" defaultValue={searchParams.get('min_price') ?? ''} placeholder="0" /></label>
      <label><span>Max price</span><input name="max_price" inputMode="decimal" defaultValue={searchParams.get('max_price') ?? ''} placeholder="NO LIMIT" /></label>
      {(['min_yield_percent', 'min_growth_percent', 'min_score'] as PickerField[]).map((field) => <PickerButton field={field} key={field} label={field === 'min_yield_percent' ? 'Yield ≥' : field === 'min_growth_percent' ? 'Growth ≥' : 'Score ≥'} value={searchParams.get(field) ?? ''} display={labelFor(field)} onClick={() => setPicker(field)} />)}
      <button type="submit">Run search <span>↗</span></button>
    </form>
    {picker && <FilterPicker field={picker} countries={countries} selected={searchParams.get(picker) ?? ''} onClose={() => setPicker(null)} onSelect={(value) => { const next = new URLSearchParams(searchParams); if (value) next.set(picker, value); else next.delete(picker); next.set('limit', '20'); setSearchParams(next); setPicker(null) }} />}
    <div className="explore-actions"><DataLabel>Results / latest normalized observations</DataLabel><div>{auth.status === 'authenticated' ? <button type="button" onClick={() => { setSaveOpen((value) => !value); setSaveMessage(null) }}>Save this search</button> : <Link to="/access" state={{ from: `/explore?${searchParams}` }}>Sign in to save</Link>}<button type="button" onClick={() => setSearchParams({ limit: '20' })}>Clear filters</button></div></div>
    {saveOpen && <form className="save-search" onSubmit={saveSearch}><label><span>Search name</span><input required maxLength={100} value={searchName} onChange={(event) => setSearchName(event.target.value)} placeholder="e.g. Lagos income watch" /></label><button type="submit">Save</button></form>}
    {saveMessage && <p className="save-message" role="status">{saveMessage}</p>}
    {properties.status === 'loading' && <ExploreState>Scanning normalized inventory…</ExploreState>}
    {properties.status === 'error' && <ExploreState error>{properties.message}</ExploreState>}
    {properties.status === 'ready' && properties.data.items.length === 0 && <ExploreState>No active properties match this market view.</ExploreState>}
    {properties.status === 'ready' && properties.data.items.length > 0 && <PropertyResults properties={properties.data.items} />}
  </main>
}

function PickerButton({ field, label, value, display, onClick }: { field: PickerField; label: string; value: string; display: string; onClick: () => void }) { return <label className="filter-picker-button"><span>{label}</span><input type="hidden" name={field} value={value} /><button type="button" onClick={onClick}>{display}<b>+</b></button></label> }
function FilterPicker({ field, countries, selected, onClose, onSelect }: { field: PickerField; countries: LocationOption[]; selected: string; onClose: () => void; onSelect: (value: string) => void }) { const options: readonly (readonly [string, string])[] = field === 'country_code' ? countries.map((country) => [country.country_code, country.name] as const) : choices[field]; const title = field === 'country_code' ? 'Choose a country' : field === 'property_type' ? 'Choose an asset type' : field === 'listing_type' ? 'Choose a listing type' : `Choose ${field.replace('min_', '').replace('_percent', '')}`; return <div className="filter-picker-layer" role="dialog" aria-modal="true" aria-label={title} onMouseDown={onClose}><section className="filter-picker-card" onMouseDown={(event) => event.stopPropagation()}><header><DataLabel>Explore filter</DataLabel><h2>{title}</h2><button type="button" onClick={onClose}>Close ×</button></header><div className="filter-picker-options"><button className={!selected ? 'is-selected' : ''} type="button" onClick={() => onSelect('')}>Any / no filter</button>{options.length ? options.map(([value, label]) => <button className={selected === value ? 'is-selected' : ''} type="button" key={value} onClick={() => onSelect(value)}>{label}</button>) : <p>No countries with normalized inventory are available yet.</p>}</div></section></div> }

function PropertyResults({ properties }: { properties: PropertyListItem[] }) {
  return <section className="property-results" aria-label="Property results"><div className="property-results__head"><span>Asset</span><span>Price</span><span>Yield</span><span>Growth</span><span>Score</span></div>{properties.map((property, index) => <Link className="property-result" to={`/properties/${property.id}`} key={property.listing_id}><div className="property-result__identity"><span>{String(index + 1).padStart(2, '0')}</span><div><strong>{property.address_line || `${property.property_type} / ${property.location_name}`}</strong><small>{property.location_name}, {property.country_code} · {property.listing_type} · {property.bedrooms ?? '—'} BED</small></div></div><span>{formatMoney(property.price, property.currency)}{property.price_period !== 'total' && <small>/{property.price_period}</small>}</span><span>{formatPercent(property.gross_yield_percent)}</span><span className={property.annual_growth_percent === null ? '' : Number(property.annual_growth_percent) >= 0 ? 'signal--positive' : 'signal--negative'}>{formatPercent(property.annual_growth_percent)}</span><span>{property.overall_score ?? '—'}</span></Link>)}</section>
}
function ExploreState({ children, error = false }: { children: string; error?: boolean }) { return <div className={`explore-state${error ? ' explore-state--error' : ''}`}>{children}</div> }
