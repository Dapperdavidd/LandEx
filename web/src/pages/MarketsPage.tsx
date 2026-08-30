import { useEffect, useState } from 'react'
import { DataLabel } from '../components/DataLabel'
import { MarketNumber } from '../components/MarketNumber'
import { useMarkets } from '../hooks/useMarkets'
import { formatMoney, formatPercent } from '../lib/format'
import { Link } from 'react-router-dom'
import { apiRequest } from '../lib/api'
import type { MarketDetail, MarketMetric } from '../types/market'
import { WorldGeometry } from '../components/WorldGeometry'
import { projectMarketPoint } from '../lib/world-geometry'

const metricOptions = ['Price', 'Yield', 'Growth', 'Demand'] as const

export function MarketsPage() {
  const markets = useMarkets(100)
  const [metric, setMetric] = useState<(typeof metricOptions)[number]>('Price')
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const selected = markets.status === 'ready' ? markets.data.items.find((market) => market.id === selectedId) ?? markets.data.items[0] : null
  const marketId = selected?.id
  const [selectedDetail, setSelectedDetail] = useState<MarketDetail | null>(null)
  useEffect(() => { if (!marketId) return; const controller = new AbortController(); apiRequest<MarketDetail>(`/markets/${marketId}?history_limit=60`, { signal: controller.signal }).then(setSelectedDetail).catch(() => setSelectedDetail(null)); return () => controller.abort() }, [marketId])

  return (
    <main className="market-page market-dashboard">
      <div className="market-dashboard__main">
      <section className="market-hero" aria-labelledby="market-title">
        <div className="market-hero__heading">
          <DataLabel>01 / Global market</DataLabel>
          <h1 id="market-title">The physical world,<br />priced.</h1>
        </div>
        <div className="market-hero__statement">
          <p>One connected view of property, place, yield and movement.</p>
          <span>Scroll to enter the market</span>
        </div>
      </section>

      <section className="market-canvas" aria-label="Global market visualization">
        <div className="market-canvas__toolbar">
          <DataLabel>World / normalized markets</DataLabel>
          <div className="metric-switcher" aria-label="Map metric">
            {metricOptions.map((option) => (
              <button className={metric === option ? 'is-active' : ''} key={option} type="button" onClick={() => setMetric(option)} disabled={option === 'Demand'}>
                {option}
              </button>
            ))}
          </div>
        </div>
        <div className="world-field" aria-label={`Market map showing ${metric.toLowerCase()} observations`}>
          <div className="world-field__grid" />
          <WorldGeometry />
          {markets.status === 'ready' && markets.data.items.filter((market) => market.latitude !== null && market.longitude !== null).map((market) => { const point = projectMarketPoint(market.longitude!, market.latitude!); return point && <button className={`market-map-point market-map-point--${metric.toLowerCase()}${selected?.id === market.id ? ' is-selected' : ''}`} style={point} onClick={() => setSelectedId(market.id)} key={market.id}><i /><span><strong>{market.location_name}</strong><small>{mapValue(metric, market)}</small></span></button> })}
          <div className="world-field__axis"><span>180°W</span><span>0°</span><span>180°E</span></div>
        </div>
        <p className="availability-note">{metric === 'Demand' ? 'Demand requires a future source-backed methodology.' : 'Points use canonical location coordinates. A point is omitted when the source location has no coordinate.'}</p>
      </section>

      <section className="market-feed" aria-labelledby="market-feed-title">
        <div className="market-feed__header">
          <div>
            <DataLabel>Observed markets</DataLabel>
            <h2 id="market-feed-title">Now in view</h2>
          </div>
          {markets.status === 'ready' && <MarketNumber compact label="NORMALIZED MARKETS" value={String(markets.data.total)} />}
          <Link className="market-compare-link" to="/compare">Compare markets ↗</Link>
        </div>

        {markets.status === 'loading' && <MarketState message="Reading the market…" />}
        {markets.status === 'error' && <MarketState message={markets.message} error />}
        {markets.status === 'ready' && markets.data.items.length === 0 && (
          <MarketState message="No normalized market observations are available yet." />
        )}
        {markets.status === 'ready' && markets.data.items.length > 0 && (
          <div className="market-table" role="table" aria-label="Latest market observations">
            <div className="market-table__head" role="row">
              <span>Market</span><span>Median price</span><span>Yield</span><span>Growth</span><span>Inventory</span>
            </div>
            {markets.data.items.slice(0, 8).map((market, index) => (
              <button className={`market-row${selected?.id === market.id ? ' is-selected' : ''}`} role="row" onClick={() => setSelectedId(market.id)} key={market.id}>
                <div className="market-row__identity">
                  <span>{String(index + 1).padStart(2, '0')}</span>
                  <div><strong>{market.location_name}</strong><small>{market.country_code} / {market.property_type ?? 'ALL PROPERTY'}</small></div>
                </div>
                <span>{formatMoney(market.latest.median_sale_price, market.latest.currency)}</span>
                <span>{formatPercent(market.latest.gross_yield_percent)}</span>
                <span className={Number(market.latest.annual_growth_percent) >= 0 ? 'signal--positive' : 'signal--negative'}>
                  {formatPercent(market.latest.annual_growth_percent)}
                </span>
                <span>{market.latest.active_inventory ?? '—'}</span>
              </button>
            ))}
          </div>
        )}
      </section>
      </div>
      <aside className="selected-market-panel" aria-label="Selected market">
        <div className="selected-market-panel__head"><DataLabel>Selected market</DataLabel>{selected && <Link to={`/markets/${selected.id}`}>Full analysis ↗</Link>}</div>
        {selected ? <><div className="selected-market-panel__visual"><div><span>{selected.location_name}</span><small>{selected.country_code} / {selected.property_type ?? 'ALL PROPERTY'}</small></div></div><div className="selected-market-panel__identity"><h2>{selected.location_name}</h2><p>{selected.country_code} · {selected.property_type ?? 'Residential market'}</p></div><div className="selected-market-panel__price"><div><DataLabel>Median observed price</DataLabel><strong>{formatMoney(selected.latest.median_sale_price, selected.latest.currency)}</strong></div><span className={Number(selected.latest.annual_growth_percent) >= 0 ? 'signal--positive' : 'signal--negative'}>{formatPercent(selected.latest.annual_growth_percent)}<small>Annual growth</small></span></div><dl className="selected-market-panel__metrics"><div><dt>Gross yield</dt><dd>{formatPercent(selected.latest.gross_yield_percent)}</dd></div><div><dt>Monthly rent</dt><dd>{formatMoney(selected.latest.median_rent_monthly, selected.latest.currency)}</dd></div><div><dt>Inventory</dt><dd>{selected.latest.active_inventory ?? '—'}</dd></div><div><dt>Observed</dt><dd>{selected.latest.observed_on ?? '—'}</dd></div></dl><div className="selected-market-panel__chart"><DataLabel>Observed price history</DataLabel>{selectedDetail ? <CompactMarketChart history={selectedDetail.history} /> : <p>Loading real observations…</p>}</div><div className="selected-market-panel__actions"><Link to={`/markets/${selected.id}`}>Study market</Link><Link to={`/explore?location_id=${selected.location_id}&limit=20`}>Explore assets</Link></div><p className="selected-market-panel__note">This is a research view from normalized observations, not an investment offering.</p></> : <div className="selected-market-panel__empty">Select a market point to study its current observations.</div>}
      </aside>
    </main>
  )
}

function mapValue(metric: (typeof metricOptions)[number], market: { latest: { median_sale_price: string | null; currency: string | null; gross_yield_percent: string | null; annual_growth_percent: string | null } }) {
  if (metric === 'Price') return formatMoney(market.latest.median_sale_price, market.latest.currency)
  if (metric === 'Yield') return formatPercent(market.latest.gross_yield_percent)
  if (metric === 'Growth') return formatPercent(market.latest.annual_growth_percent)
  return 'Unavailable'
}

function MarketState({ message, error = false }: { message: string; error?: boolean }) {
  return <div className={`market-state${error ? ' market-state--error' : ''}`}>{message}</div>
}
function CompactMarketChart({ history }: { history: MarketMetric[] }) { const points = [...history].reverse(); const values = points.map((point) => Number(point.median_sale_price)).filter(Number.isFinite); if (values.length < 2) return <p>More verified observations are needed before a trend can be drawn.</p>; const min = Math.min(...values), max = Math.max(...values), spread = max - min || 1; const path = values.map((value, index) => `${index ? 'L' : 'M'} ${(index / (values.length - 1)) * 100} ${92 - ((value - min) / spread) * 78}`).join(' '); return <svg viewBox="0 0 100 100" preserveAspectRatio="none" role="img" aria-label="Observed market price history"><path className="compact-market-chart__area" d={`${path} L 100 100 L 0 100 Z`} /><path className="compact-market-chart__line" d={path} /></svg> }
