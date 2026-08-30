import { useState } from 'react'
import { Link, useLocation, useParams } from 'react-router-dom'
import { useAuth } from '../auth/auth-context'
import { DataLabel } from '../components/DataLabel'
import { PropertyHistoryChart } from '../components/PropertyHistoryChart'
import { usePropertyDetail } from '../hooks/usePropertyDetail'
import { apiRequest } from '../lib/api'
import { formatMoney, formatPercent } from '../lib/format'
import type { PropertyListItem, WatchlistSummary } from '../types/property'

export function PropertyPage() {
  const { id } = useParams(); const route = useLocation(); const auth = useAuth(); const detail = usePropertyDetail(id)
  const [watchState, setWatchState] = useState<'idle' | 'saving' | 'saved'>('idle'); const [actionError, setActionError] = useState<string | null>(null)
  const [activeMedia, setActiveMedia] = useState(0)
  if (detail.status === 'loading') return <PropertyState>Loading asset intelligence…</PropertyState>
  if (detail.status === 'error') return <PropertyState error>{detail.message}</PropertyState>
  const { property, history, score, location } = detail.data
  const title = property.address_line || `${property.property_type} / ${property.location_name}`
  async function saveToWatchlist() {
    if (!auth.accessToken || !id) return
    setWatchState('saving'); setActionError(null)
    try {
      let lists = await apiRequest<WatchlistSummary[]>('/watchlists', { headers: { Authorization: `Bearer ${auth.accessToken}` } })
      if (!lists.length) lists = [await apiRequest<WatchlistSummary>('/watchlists', { method: 'POST', headers: { Authorization: `Bearer ${auth.accessToken}` }, body: JSON.stringify({ name: 'My Watchlist' }) })]
      const watchlist = lists.at(0)
      if (!watchlist) throw new Error('A watchlist could not be created.')
      await apiRequest(`/watchlists/${watchlist.id}/items`, { method: 'POST', headers: { Authorization: `Bearer ${auth.accessToken}` }, body: JSON.stringify({ target_type: 'property', target_id: id }) })
      setWatchState('saved')
    } catch (error) { setWatchState('idle'); setActionError(error instanceof Error ? error.message : 'Property could not be saved.') }
  }
  return <main className="property-page">
    <header className="asset-header"><div><DataLabel>03 / Property asset</DataLabel><p>{property.location_name} / {property.country_code} / {property.property_type}</p><h1>{title}</h1></div><div className="asset-header__quote"><DataLabel>Current asking price</DataLabel><strong>{formatMoney(property.price, property.currency)}</strong><span className={property.annual_growth_percent === null ? '' : Number(property.annual_growth_percent) >= 0 ? 'signal--positive' : 'signal--negative'}>{formatPercent(property.annual_growth_percent)} / 12M</span></div></header>
    <section className={`asset-media${property.media_urls.length ? '' : ' asset-media--empty'}`} aria-label="Provider property media">{property.media_urls.length ? <><img src={property.media_urls[activeMedia] ?? property.media_urls[0]} alt={`${title} supplied by ${property.source_name}`} referrerPolicy="no-referrer" /><div className="asset-media__index"><DataLabel>Provider media / {activeMedia + 1} of {property.media_urls.length}</DataLabel>{property.media_urls.length > 1 && <div>{property.media_urls.slice(0, 8).map((url, index) => <button className={index === activeMedia ? 'is-active' : ''} type="button" onClick={() => setActiveMedia(index)} key={url} aria-label={`Show provider image ${index + 1}`} />)}</div>}</div></> : <div><DataLabel>Provider media unavailable</DataLabel><strong>{property.location_name}</strong><p>LandEX has no licensed image for this record. The asset data remains available for research.</p></div>}</section>
    <section className="asset-actions" aria-label="Property actions">
      {auth.status === 'authenticated' ? <button type="button" onClick={saveToWatchlist} disabled={watchState !== 'idle'}>{watchState === 'saved' ? 'Saved to watchlist' : watchState === 'saving' ? 'Saving…' : 'Add to watchlist'}</button> : <Link to="/access" state={{ from: route.pathname }}>Sign in to watch</Link>}
      <Link className="asset-actions__primary" to={auth.status === 'authenticated' ? `/simulate?property=${property.id}` : '/access'} state={auth.status === 'authenticated' ? undefined : { from: `/simulate?property=${property.id}` }}>Paper invest ↗</Link>
      {property.source_url && <a href={property.source_url} target="_blank" rel="noreferrer">View source ↗</a>}
      <span className="asset-source">Source / {property.source_name}</span>
      {actionError && <span role="alert">{actionError}</span>}
    </section>
    <section className="asset-metrics"><Metric label="Rental yield" value={formatPercent(property.gross_yield_percent)} /><Metric label="Annual growth" value={formatPercent(property.annual_growth_percent)} signal /><Metric label="Investment score" value={score.overall_score ?? '—'} /><Metric label="Last observed" value={new Date(property.last_seen_at).toLocaleDateString()} /></section>
    <InvestmentBrief property={property} unavailable={score.unavailable_components} />
    <section className="asset-chart"><div className="section-heading"><div><DataLabel>Market history</DataLabel><h2>Price as a signal.</h2></div><span>{history.length} observations</span></div><PropertyHistoryChart points={history} /></section>
    <section className="intelligence-grid">
      <div className="score-panel"><div className="section-heading"><div><DataLabel>Investment profile</DataLabel><h2>Explain the score.</h2></div><strong>{score.overall_score ?? '—'}</strong></div><div className="score-components">{score.components.map((component) => <details key={component.name}><summary><span>{component.name}</span><strong>{component.score ?? 'N/A'}</strong></summary><p>{component.methodology}</p></details>)}</div><p className="method-note">Unavailable components remain excluded rather than guessed: {score.unavailable_components.join(', ') || 'none'}.</p></div>
      <div className="location-panel"><div className="section-heading"><div><DataLabel>Location intelligence / {location.radius_meters / 1000}KM</DataLabel><h2>What surrounds it.</h2></div><span>{location.cache.fresh ? 'Fresh cache' : 'Cached data'}</span></div>{location.categories.length ? <div className="location-categories">{location.categories.map((category) => <div key={category.category}><strong>{category.feature_count}</strong><span>{category.category}</span><small>Nearest {category.nearest_distance_meters}m</small></div>)}</div> : <div className="location-empty">No cached surroundings are available for this property yet.</div>}<div className="nearby-list">{location.features.slice(0, 6).map((feature) => <div key={feature.id}><span>{feature.name || feature.kind}</span><small>{feature.category} / {feature.distance_meters}m</small></div>)}</div></div>
    </section>
    <section className="asset-facts"><DataLabel>Asset facts</DataLabel><dl><Fact label="Bedrooms" value={property.bedrooms} /><Fact label="Bathrooms" value={property.bathrooms} /><Fact label="Area" value={property.area_sqm ? `${property.area_sqm} m²` : null} /><Fact label="Year built" value={property.year_built?.toString()} /><Fact label="Postal code" value={property.postal_code} /><Fact label="Coordinates" value={property.latitude !== null && property.longitude !== null ? `${property.latitude.toFixed(4)}, ${property.longitude.toFixed(4)}` : null} /></dl></section>
  </main>
}

function Metric({ label, value, signal = false }: { label: string; value: string; signal?: boolean }) { return <div><DataLabel>{label}</DataLabel><strong className={signal && value !== '—' ? Number.parseFloat(value) >= 0 ? 'signal--positive' : 'signal--negative' : ''}>{value}</strong></div> }
function InvestmentBrief({ property, unavailable }: { property: PropertyListItem; unavailable: string[] }) {
  const yieldValue = property.gross_yield_percent === null ? null : Number(property.gross_yield_percent)
  const growthValue = property.annual_growth_percent === null ? null : Number(property.annual_growth_percent)
  return <section className="investment-brief"><div><DataLabel>Before you paper invest</DataLabel><h2>Understand the signals.</h2><p>LandEX separates what the source observed from what its calculation engine derived. This is educational research, not a promise of future return.</p></div><div className="investment-brief__lessons"><article><span>01 / Income</span><strong>{yieldValue === null ? 'Not enough rent data' : `${yieldValue.toFixed(2)}% gross yield`}</strong><p>{yieldValue === null ? 'A yield is withheld until both price and rent inputs are available.' : 'Gross yield compares estimated annual rent with asking price before vacancy, tax, maintenance, insurance, financing, and fees.'}</p></article><article><span>02 / Market movement</span><strong>{growthValue === null ? 'Growth unavailable' : `${growthValue >= 0 ? '+' : ''}${growthValue.toFixed(2)}% annual growth`}</strong><p>{growthValue === null ? 'The surrounding market needs a verified annual-growth observation.' : 'This is the latest normalized market observation, not a forecast of what this property will earn.'}</p></article><article><span>03 / Confidence</span><strong>{unavailable.length ? `${unavailable.length} inputs unavailable` : 'All score inputs available'}</strong><p>Missing score components are excluded rather than estimated. Open the profile below to inspect each methodology.</p></article></div></section>
}
function Fact({ label, value }: { label: string; value: string | null | undefined }) { return <div><dt>{label}</dt><dd>{value ?? 'Unavailable'}</dd></div> }
function PropertyState({ children, error = false }: { children: string; error?: boolean }) { return <main className={`property-state${error ? ' property-state--error' : ''}`}>{children}<Link to="/explore">Return to Explore</Link></main> }
