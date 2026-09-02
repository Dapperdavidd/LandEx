import { useEffect, useMemo, useState, type FormEvent } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useAuth } from '../auth/auth-context'
import { DataLabel } from '../components/DataLabel'
import { useProperties } from '../hooks/useProperties'
import { useInstruments } from '../hooks/useInstruments'
import { apiRequest } from '../lib/api'
import { formatMoney, formatPercent } from '../lib/format'
import type { PropertyListItem, SavedSearch } from '../types/property'
import type { InstrumentDetail, InvestmentInstrument } from '../types/instrument'

const fields = ['country_code', 'location_id', 'currency', 'property_type', 'listing_type', 'min_price', 'max_price', 'min_yield_percent', 'min_growth_percent', 'min_score'] as const
const choices = { currency: [['USD', 'US dollar'], ['NGN', 'Nigerian naira'], ['GBP', 'British pound'], ['EUR', 'Euro'], ['AED', 'UAE dirham'], ['CAD', 'Canadian dollar'], ['AUD', 'Australian dollar']], property_type: [['apartment', 'Apartment'], ['house', 'House'], ['commercial', 'Commercial'], ['land', 'Land'], ['hotel', 'Hotel'], ['retail', 'Retail'], ['industrial', 'Industrial']], listing_type: [['sale', 'For sale'], ['rent', 'For rent'], ['shortlet', 'Shortlet']], min_yield_percent: [['4', '4%+'], ['6', '6%+'], ['8', '8%+'], ['10', '10%+']], min_growth_percent: [['3', '3%+'], ['5', '5%+'], ['8', '8%+'], ['12', '12%+']], min_score: [['50', '50+'], ['65', '65+'], ['75', '75+'], ['85', '85+']] } as const
type PickerField = keyof typeof choices | 'country_code' | 'location_id'
type LocationOption = { id: string; name: string; country_code: string; property_count: number }

export function ExplorePage() {
  const auth = useAuth()
  const [searchParams, setSearchParams] = useSearchParams({ limit: '20' })
  const query = useMemo(() => new URLSearchParams(searchParams), [searchParams])
  const properties = useProperties(query)
  const requestedView = searchParams.get('view')
  const view = requestedView === 'opportunities' || requestedView === 'listed' ? requestedView : 'markets'
  const [saveOpen, setSaveOpen] = useState(false)
  const [searchName, setSearchName] = useState('')
  const [saveMessage, setSaveMessage] = useState<string | null>(null)
  const [picker, setPicker] = useState<PickerField | null>(null)
  const [countries, setCountries] = useState<LocationOption[]>([])
  const [cities, setCities] = useState<LocationOption[]>([])
  useEffect(() => { const controller = new AbortController(); apiRequest<LocationOption[]>('/locations?kind=country&limit=300', { signal: controller.signal }).then(setCountries).catch(() => setCountries([])); return () => controller.abort() }, [])
  const selectedCountry = searchParams.get('country_code') ?? ''
  useEffect(() => { if (!selectedCountry) return; const controller = new AbortController(); apiRequest<LocationOption[]>(`/locations?kind=city&country_code=${selectedCountry}&limit=100`, { signal: controller.signal }).then(setCities).catch(() => setCities([])); return () => controller.abort() }, [selectedCountry])
  const visibleCities = selectedCountry ? cities : []
  const labelFor = (field: PickerField) => { const value = searchParams.get(field); if (!value) return field === 'country_code' ? 'Global' : 'Any'; if (field === 'country_code') return countries.find((country) => country.country_code === value)?.name ?? value; if (field === 'location_id') return visibleCities.find((city) => city.id === value)?.name ?? 'Any city'; return choices[field].find(([key]) => key === value)?.[1] ?? value }

  function applyFilters(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const form = new FormData(event.currentTarget)
    const next = new URLSearchParams({ view: 'opportunities', limit: '20' })
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
    <header className="explore-header"><div><DataLabel>02 / Global research universe</DataLabel><h1>Find an<br />opportunity.</h1></div><div className="explore-header__aside"><span>{view === 'opportunities' && properties.status === 'ready' ? properties.data.total : view === 'listed' ? 'SEC' : '57'}</span><DataLabel>{view === 'opportunities' ? 'Observed active listings' : view === 'listed' ? 'Verified listed identities' : 'Countries with official BIS history'}</DataLabel></div></header>
    <nav className="universe-switch" aria-label="Research universe"><button className={view === 'markets' ? 'is-active' : ''} type="button" onClick={() => setSearchParams({ view: 'markets', limit: '100' })}>Market instruments <small>Verified price history</small></button><button className={view === 'listed' ? 'is-active' : ''} type="button" onClick={() => setSearchParams({ view: 'listed', limit: '100' })}>Listed real estate <small>SEC-verified identities</small></button><button className={view === 'opportunities' ? 'is-active' : ''} type="button" onClick={() => setSearchParams({ view: 'opportunities', limit: '20' })}>Property research <small>Active listings, not offerings</small></button></nav>
    {view === 'markets' ? <MarketInstrumentUniverse /> : view === 'listed' ? <ListedInstrumentUniverse /> : <>
    <form className="filter-terminal" onSubmit={applyFilters}>
      {(['country_code', 'location_id', 'currency', 'property_type', 'listing_type'] as PickerField[]).map((field) => <PickerButton field={field} key={field} label={field === 'country_code' ? 'Country' : field === 'location_id' ? 'City' : field === 'property_type' ? 'Asset' : field === 'listing_type' ? 'Listing' : 'Currency'} value={searchParams.get(field) ?? ''} display={labelFor(field)} onClick={() => setPicker(field)} />)}
      <label><span>Min price</span><input name="min_price" inputMode="decimal" defaultValue={searchParams.get('min_price') ?? ''} placeholder="0" /></label>
      <label><span>Max price</span><input name="max_price" inputMode="decimal" defaultValue={searchParams.get('max_price') ?? ''} placeholder="NO LIMIT" /></label>
      {(['min_yield_percent', 'min_growth_percent', 'min_score'] as PickerField[]).map((field) => <PickerButton field={field} key={field} label={field === 'min_yield_percent' ? 'Yield ≥' : field === 'min_growth_percent' ? 'Growth ≥' : 'Score ≥'} value={searchParams.get(field) ?? ''} display={labelFor(field)} onClick={() => setPicker(field)} />)}
      <button type="submit">Run search <span>↗</span></button>
    </form>
    {picker && <FilterPicker field={picker} countries={countries} cities={visibleCities} selected={searchParams.get(picker) ?? ''} onClose={() => setPicker(null)} onSelect={(value) => { const next = new URLSearchParams(searchParams); if (value) next.set(picker, value); else next.delete(picker); if (picker === 'country_code') next.delete('location_id'); next.set('limit', '20'); setSearchParams(next); setPicker(null) }} />}
    <div className="explore-actions"><DataLabel>Results / latest normalized observations</DataLabel><div>{auth.status === 'authenticated' ? <button type="button" onClick={() => { setSaveOpen((value) => !value); setSaveMessage(null) }}>Save this search</button> : <Link to="/access" state={{ from: `/explore?${searchParams}` }}>Sign in to save</Link>}<button type="button" onClick={() => setSearchParams({ view: 'opportunities', limit: '20' })}>Clear filters</button></div></div>
    {saveOpen && <form className="save-search" onSubmit={saveSearch}><label><span>Search name</span><input required maxLength={100} value={searchName} onChange={(event) => setSearchName(event.target.value)} placeholder="e.g. Lagos income watch" /></label><button type="submit">Save</button></form>}
    {saveMessage && <p className="save-message" role="status">{saveMessage}</p>}
    {properties.status === 'loading' && <ExploreState>Scanning normalized inventory…</ExploreState>}
    {properties.status === 'error' && <ExploreState error>{properties.message}</ExploreState>}
    {properties.status === 'ready' && properties.data.items.length === 0 && <ExploreState>No active properties match this market view.</ExploreState>}
    {properties.status === 'ready' && properties.data.items.length > 0 && <PropertyResults properties={properties.data.items} />}
    </>}
  </main>
}

function ListedInstrumentUniverse() {
  const instruments = useInstruments('', 100, 'sec-edgar-listed-reits')
  if (instruments.status === 'loading') return <ExploreState>Loading SEC-verified listed real-estate identities…</ExploreState>
  if (instruments.status === 'error') return <ExploreState error>{instruments.message}</ExploreState>
  return <section className="instrument-universe listed-universe"><div className="instrument-universe__intro"><div><DataLabel>SEC EDGAR / listed real estate</DataLabel><h2>Public-market real estate,<br />without invented quotes.</h2></div><p>Issuer names, tickers, and exchanges are verified against the SEC public catalogue. Quote history is not yet connected, so these instruments remain research-only and cannot be paper traded. Open the filing source to verify each identity.</p></div><div className="listed-ledger"><div className="listed-ledger__head"><span>Issuer</span><span>Symbol</span><span>Exchange</span><span>Coverage</span><span>Source</span></div>{instruments.data.items.map((instrument) => <article key={instrument.id}><div><strong>{instrument.name}</strong><small>LISTED SECURITY · RESEARCH ONLY · NO QUOTE SERIES</small></div><b>{instrument.symbol ?? '—'}</b><span>{instrument.exchange ?? '—'}</span><span>Identity verified</span>{instrument.source_url ? <a href={instrument.source_url} target="_blank" rel="noreferrer">SEC filing ↗</a> : <span>—</span>}</article>)}</div><p className="listed-universe__truth">Showing {instruments.data.items.length} of {instruments.data.total} conservative matches whose SEC issuer names explicitly identify them as REITs. This is not a complete U.S. REIT universe, a recommendation, or a real-money offering.</p></section>
}

function MarketInstrumentUniverse() {
  const instruments = useInstruments('', 100, 'bis-property-prices')
  const [selected, setSelected] = useState<InvestmentInstrument | null>(null)
  const [loadedDetail, setLoadedDetail] = useState<{ id: string; value: InstrumentDetail } | null>(null)
  const effectiveSelected = selected ?? (instruments.status === 'ready' ? instruments.data.items[0] ?? null : null)
  useEffect(() => {
    if (!effectiveSelected) return
    const controller = new AbortController()
    const id = effectiveSelected.id
    apiRequest<InstrumentDetail>(`/instruments/${id}?history_limit=120`, { signal: controller.signal }).then((value) => setLoadedDetail({ id, value })).catch(() => undefined)
    return () => controller.abort()
  }, [effectiveSelected])
  if (instruments.status === 'loading') return <ExploreState>Loading official market instruments…</ExploreState>
  if (instruments.status === 'error') return <ExploreState error>{instruments.message}</ExploreState>
  const detail = loadedDetail && loadedDetail.id === effectiveSelected?.id ? loadedDetail.value : null
  return <section className="instrument-universe"><div className="instrument-universe__intro"><div><DataLabel>BIS / 57 country markets</DataLabel><h2>Markets you can study<br />and paper trade.</h2></div><p>These are comparable nationwide market proxies, not properties for sale. Values follow the official BIS selected nominal residential price indices. Real-money investing is disabled.</p></div><div className="instrument-terminal"><div className="instrument-ledger"><div className="instrument-ledger__head"><span>Instrument</span><span>Index value</span><span>12M</span><span>Observed</span></div>{instruments.data.items.map((instrument) => <button className={effectiveSelected?.id === instrument.id ? 'is-selected' : ''} type="button" key={instrument.id} onClick={() => setSelected(instrument)}><div><strong>{instrument.name}</strong><small>MARKET PROXY · {instrument.country_code} · QUARTERLY · PAPER</small></div><span>{formatInstrumentValue(instrument)}</span><span className={Number(instrument.annual_change_percent) >= 0 ? 'signal--positive' : 'signal--negative'}>{formatPercent(instrument.annual_change_percent)}</span><span>{instrument.observed_on ?? '—'}</span></button>)}</div><aside className="instrument-inspector">{effectiveSelected ? <><DataLabel>Selected instrument</DataLabel><h3>{effectiveSelected.name}</h3><p>{effectiveSelected.valuation_method}</p><div className="instrument-inspector__quote"><strong>{formatInstrumentValue(effectiveSelected)}</strong><span className={Number(effectiveSelected.annual_change_percent) >= 0 ? 'signal--positive' : 'signal--negative'}>{formatPercent(effectiveSelected.annual_change_percent)}<small>12 month change</small></span></div><dl><div><dt>Classification</dt><dd>Market proxy</dd></div><div><dt>Frequency</dt><dd>Quarterly</dd></div><div><dt>Mode</dt><dd>Paper only</dd></div><div><dt>Real money</dt><dd>Disabled</dd></div></dl><div className="instrument-inspector__chart"><DataLabel>Official index history / recent 120 periods</DataLabel>{detail ? <InstrumentChart detail={detail} /> : <p>Loading verified history…</p>}</div>{effectiveSelected.source_url && <a href={effectiveSelected.source_url} target="_blank" rel="noreferrer">Verify official source ↗</a>}</> : null}</aside></div></section>
}

function formatInstrumentValue(instrument: InvestmentInstrument) {
  if (instrument.metadata.value_kind === 'index') return instrument.value === null ? '—' : `${Number(instrument.value).toFixed(1)} pts`
  return formatMoney(instrument.value, instrument.currency)
}

function InstrumentChart({ detail }: { detail: InstrumentDetail }) {
  const history = [...detail.history].reverse()
  const values = history.map((point) => Number(point.value)).filter(Number.isFinite)
  if (values.length < 2) return <p>More observations are required.</p>
  const min = Math.min(...values), max = Math.max(...values), spread = max - min || 1
  const path = values.map((value, index) => `${index ? 'L' : 'M'} ${(index / (values.length - 1)) * 100} ${92 - ((value - min) / spread) * 78}`).join(' ')
  return <><svg viewBox="0 0 100 100" preserveAspectRatio="none" role="img" aria-label="Official market price history"><path className="instrument-chart__area" d={`${path} L 100 100 L 0 100 Z`} /><path className="instrument-chart__line" d={path} /></svg><div><span>{history[0]?.observed_on.slice(0, 4)}</span><span>{history.at(-1)?.observed_on.slice(0, 4)}</span></div></>
}

function PickerButton({ field, label, value, display, onClick }: { field: PickerField; label: string; value: string; display: string; onClick: () => void }) { return <label className="filter-picker-button"><span>{label}</span><input type="hidden" name={field} value={value} /><button type="button" onClick={onClick}>{display}<b>+</b></button></label> }
function FilterPicker({ field, countries, cities, selected, onClose, onSelect }: { field: PickerField; countries: LocationOption[]; cities: LocationOption[]; selected: string; onClose: () => void; onSelect: (value: string) => void }) { const options: readonly (readonly [string, string])[] = field === 'country_code' ? countries.map((country) => [country.country_code, country.name] as const) : field === 'location_id' ? cities.map((city) => [city.id, city.name] as const) : choices[field]; const title = field === 'country_code' ? 'Choose a country' : field === 'location_id' ? 'Choose a city' : field === 'property_type' ? 'Choose an asset type' : field === 'listing_type' ? 'Choose a listing type' : `Choose ${field.replace('min_', '').replace('_percent', '')}`; return <div className="filter-picker-layer" role="dialog" aria-modal="true" aria-label={title} onMouseDown={onClose}><section className="filter-picker-card" onMouseDown={(event) => event.stopPropagation()}><header><DataLabel>Explore filter</DataLabel><h2>{title}</h2><button type="button" onClick={onClose}>Close ×</button></header><div className="filter-picker-options"><button className={!selected ? 'is-selected' : ''} type="button" onClick={() => onSelect('')}>Any / no filter</button>{options.length ? options.map(([value, label]) => <button className={selected === value ? 'is-selected' : ''} type="button" key={value} onClick={() => onSelect(value)}>{label}</button>) : <p>{field === 'location_id' ? 'Choose a country first.' : 'No normalized options are available yet.'}</p>}</div></section></div> }

function PropertyResults({ properties }: { properties: PropertyListItem[] }) {
  return <section className="property-results" aria-label="Property results"><div className="property-results__head"><span>Observed property / research only</span><span>Listing price</span><span>Yield</span><span>Growth</span><span>Score</span></div>{properties.map((property, index) => <Link className="property-result" to={`/properties/${property.id}`} key={property.listing_id}>{property.media_urls[0] && <span className="property-result__preview" aria-hidden="true"><img src={property.media_urls[0]} alt="" referrerPolicy="no-referrer" /><small>Provider media / {property.source_name}</small></span>}<div className="property-result__identity"><span>{String(index + 1).padStart(2, '0')}</span><div><strong>{property.address_line || `${property.property_type} / ${property.location_name}`}</strong><small>ACTIVE LISTING · NOT AN OFFERING · {property.location_name}, {property.country_code} · {property.source_name}</small></div></div><span>{formatMoney(property.price, property.currency)}{property.price_period !== 'total' && <small>/{property.price_period}</small>}</span><span>{formatPercent(property.gross_yield_percent)}</span><span className={property.annual_growth_percent === null ? '' : Number(property.annual_growth_percent) >= 0 ? 'signal--positive' : 'signal--negative'}>{formatPercent(property.annual_growth_percent)}</span><span>{property.overall_score ?? '—'}</span></Link>)}</section>
}
function ExploreState({ children, error = false }: { children: string; error?: boolean }) { return <div className={`explore-state${error ? ' explore-state--error' : ''}`}>{children}</div> }
