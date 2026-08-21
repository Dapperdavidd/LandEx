import { useState } from 'react'
import { DataLabel } from '../components/DataLabel'
import { MarketNumber } from '../components/MarketNumber'
import { useMarkets } from '../hooks/useMarkets'
import { formatMoney, formatPercent } from '../lib/format'
import { Link } from 'react-router-dom'

const metricOptions = ['Price', 'Yield', 'Growth', 'Demand'] as const

export function MarketsPage() {
  const markets = useMarkets(100)
  const [metric, setMetric] = useState<(typeof metricOptions)[number]>('Price')

  return (
    <main className="market-page">
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
          <span className="continent continent--americas">AMERICAS</span><span className="continent continent--europe">EUROPE</span><span className="continent continent--africa">AFRICA</span><span className="continent continent--asia">ASIA</span>
          {markets.status === 'ready' && markets.data.items.filter((market) => market.latitude !== null && market.longitude !== null).map((market) => <Link className={`market-map-point market-map-point--${metric.toLowerCase()}`} style={{ left: `${((market.longitude! + 180) / 360) * 100}%`, top: `${((90 - market.latitude!) / 180) * 100}%` }} to={`/markets/${market.id}`} key={market.id}><i /><span><strong>{market.location_name}</strong><small>{mapValue(metric, market)}</small></span></Link>)}
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
              <Link className="market-row" role="row" to={`/markets/${market.id}`} key={market.id}>
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
              </Link>
            ))}
          </div>
        )}
      </section>
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
