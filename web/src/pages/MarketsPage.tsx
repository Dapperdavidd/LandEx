import { DataLabel } from '../components/DataLabel'
import { MarketNumber } from '../components/MarketNumber'
import { useMarkets } from '../hooks/useMarkets'
import { formatMoney, formatPercent } from '../lib/format'
import { Link } from 'react-router-dom'

const metricOptions = ['Price', 'Yield', 'Growth', 'Demand'] as const

export function MarketsPage() {
  const markets = useMarkets()

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
            {metricOptions.map((metric, index) => (
              <button className={index === 0 ? 'is-active' : ''} key={metric} type="button" disabled={index > 0}>
                {metric}
              </button>
            ))}
          </div>
        </div>
        <div className="world-field" aria-hidden="true">
          <div className="world-field__grid" />
          <span className="continent continent--americas">AMERICAS</span>
          <span className="continent continent--europe">EUROPE</span>
          <span className="continent continent--africa">AFRICA</span>
          <span className="continent continent--asia">ASIA</span>
          <i className="market-point market-point--one" />
          <i className="market-point market-point--two" />
          <i className="market-point market-point--three" />
          <i className="market-point market-point--four" />
          <div className="world-field__axis"><span>180°W</span><span>0°</span><span>180°E</span></div>
        </div>
        <p className="availability-note">Map geometry is the next integration layer. No global market values are fabricated.</p>
      </section>

      <section className="market-feed" aria-labelledby="market-feed-title">
        <div className="market-feed__header">
          <div>
            <DataLabel>Observed markets</DataLabel>
            <h2 id="market-feed-title">Now in view</h2>
          </div>
          {markets.status === 'ready' && <MarketNumber compact label="NORMALIZED MARKETS" value={String(markets.data.total)} />}
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
            {markets.data.items.map((market, index) => (
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

function MarketState({ message, error = false }: { message: string; error?: boolean }) {
  return <div className={`market-state${error ? ' market-state--error' : ''}`}>{message}</div>
}
