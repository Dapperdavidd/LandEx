import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { useAuth } from '../auth/auth-context'
import { DataLabel } from '../components/DataLabel'
import { PortfolioChart } from '../components/PortfolioChart'
import { apiRequest } from '../lib/api'
import { formatMoney, formatPercent } from '../lib/format'
import type { PaperAccount, PaperTrade, PortfolioAllocation, PortfolioPerformance, PortfolioSnapshot } from '../types/paper'

interface PortfolioData { accounts: PaperAccount[]; account: PaperAccount; performance: PortfolioPerformance; allocation: PortfolioAllocation; history: PortfolioSnapshot[]; trades: PaperTrade[] }
type PortfolioState = { status: 'loading' } | { status: 'empty' } | { status: 'error'; message: string } | { status: 'ready'; data: PortfolioData }
export function PortfolioPage() {
  const auth = useAuth(); const token = auth.accessToken; const [selectedId, setSelectedId] = useState<string | null>(null); const [state, setState] = useState<PortfolioState>({ status: 'loading' })
  useEffect(() => {
    if (!token) return
    const controller = new AbortController(); const headers = { Authorization: `Bearer ${token}` }
    apiRequest<PaperAccount[]>('/paper-accounts', { headers, signal: controller.signal }).then(async (accounts) => {
      const account = accounts.find((candidate) => candidate.id === selectedId) ?? accounts.at(0)
      if (!account) { setState({ status: 'empty' }); return }
      const [performance, allocation, history, trades] = await Promise.all([
        apiRequest<PortfolioPerformance>(`/paper-accounts/${account.id}/performance`, { headers, signal: controller.signal }),
        apiRequest<PortfolioAllocation>(`/paper-accounts/${account.id}/allocation`, { headers, signal: controller.signal }),
        apiRequest<PortfolioSnapshot[]>(`/paper-accounts/${account.id}/performance-history`, { headers, signal: controller.signal }),
        apiRequest<PaperTrade[]>(`/paper-accounts/${account.id}/trades`, { headers, signal: controller.signal }),
      ]); setState({ status: 'ready', data: { accounts, account, performance, allocation, history, trades } })
    }).catch((error: unknown) => { if (error instanceof DOMException && error.name === 'AbortError') return; setState({ status: 'error', message: error instanceof Error ? error.message : 'Portfolio intelligence is unavailable.' }) })
    return () => controller.abort()
  }, [token, selectedId])
  if (state.status === 'loading') return <PortfolioState>Marking positions to current real-estate data…</PortfolioState>
  if (state.status === 'error') return <PortfolioState error>{state.message}</PortfolioState>
  if (state.status === 'empty') return <main className="portfolio-empty"><DataLabel>04 / Portfolio</DataLabel><h1>No positions.<br />Build a view.</h1><p>Paper-invest in a property to create your first demo portfolio.</p><Link to="/explore">Explore opportunities ↗</Link></main>
  const { accounts, account, performance, allocation, history, trades } = state.data; const returnPositive = Number(performance.total_return_percent) >= 0
  return <main className="portfolio-page"><header className="portfolio-header"><div><DataLabel>04 / Paper portfolio</DataLabel><h1>{formatMoney(performance.total_value, performance.base_currency)}</h1><span className={returnPositive ? 'signal--positive' : 'signal--negative'}>{formatPercent(performance.total_return_percent)} / ALL TIME</span></div><label><span>Portfolio account</span><select value={account.id} onChange={(event) => setSelectedId(event.target.value)}>{accounts.map((item) => <option value={item.id} key={item.id}>{item.name} / {item.base_currency}</option>)}</select></label></header><section className="portfolio-metrics"><Metric label="Cash" value={formatMoney(performance.cash_balance, performance.base_currency)} /><Metric label="Positions" value={formatMoney(performance.positions_value, performance.base_currency)} /><Metric label="Total P&L" value={formatMoney(performance.total_pnl, performance.base_currency)} signal={Number(performance.total_pnl)} /><Metric label="Realized P&L" value={formatMoney(performance.realized_pnl, performance.base_currency)} signal={Number(performance.realized_pnl)} /></section><section className="portfolio-performance"><div className="section-heading"><div><DataLabel>Performance / daily snapshots</DataLabel><h2>Capital over time.</h2></div><span>{history.length} observations</span></div><PortfolioChart snapshots={history} /></section><section className="portfolio-positions"><div className="section-heading"><div><DataLabel>Open positions</DataLabel><h2>Current exposure.</h2></div><span>{performance.positions.length} assets</span></div><div className="positions-ledger"><div><span>Asset</span><span>Market value</span><span>Entry</span><span>Return</span></div>{performance.positions.map((position, index) => <Link to={`/properties/${position.property_id}`} key={position.property_id}><span><small>{String(index + 1).padStart(2, '0')}</small><strong>{position.property_type} / {position.country_code}</strong></span><span>{formatMoney(position.market_value, position.currency)}</span><span>{formatMoney(position.average_entry_price, position.currency)}</span><span className={Number(position.return_percent) >= 0 ? 'signal--positive' : 'signal--negative'}>{formatPercent(position.return_percent)}</span></Link>)}</div></section><section className="portfolio-lower"><div><DataLabel>Allocation / country</DataLabel><Allocation items={allocation.by_country} /><DataLabel>Allocation / asset type</DataLabel><Allocation items={allocation.by_property_type} /><p>Unavailable dimensions are not estimated: {allocation.unavailable_dimensions.join(', ')}.</p></div><div><DataLabel>Trade activity</DataLabel><div className="trade-ledger">{trades.length ? trades.slice(0, 10).map((trade) => <div key={trade.id}><span className={trade.side === 'buy' ? 'signal--positive' : 'signal--negative'}>{trade.side}</span><Link to={`/properties/${trade.property_id}`}>{formatMoney(trade.gross_amount, trade.currency)}</Link><small>{new Date(trade.executed_at).toLocaleDateString()}</small></div>) : <p>No trades have been executed in this account.</p>}</div></div></section></main>
}
function Metric({ label, value, signal }: { label: string; value: string; signal?: number }) { return <div><DataLabel>{label}</DataLabel><strong className={signal === undefined ? '' : signal >= 0 ? 'signal--positive' : 'signal--negative'}>{value}</strong></div> }
function Allocation({ items }: { items: PortfolioAllocation['by_country'] }) { return <div className="allocation-list">{items.length ? items.map((item) => <div key={item.label}><span>{item.label}</span><i><b style={{ width: `${Math.min(100, Number(item.percentage))}%` }} /></i><strong>{formatPercent(item.percentage)}</strong></div>) : <p>No invested positions yet.</p>}</div> }
function PortfolioState({ children, error = false }: { children: string; error?: boolean }) { return <main className={`portfolio-state${error ? ' portfolio-state--error' : ''}`}>{children}</main> }
