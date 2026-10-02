import { useEffect, useState, type FormEvent } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useAuth } from '../auth/auth-context'
import { DataLabel } from '../components/DataLabel'
import { apiRequest } from '../lib/api'
import { formatMoney, formatPercent } from '../lib/format'
import type { InstrumentDetail } from '../types/instrument'
import type { InstrumentPaperTrade, InstrumentPortfolioPerformance, PaperAccount } from '../types/paper'

type ReadyState = { status: 'ready'; instrument: InstrumentDetail; account: PaperAccount; performance: InstrumentPortfolioPerformance }
type State = { status: 'loading' } | { status: 'error'; message: string } | { status: 'empty' } | ReadyState
type OrderSide = 'buy' | 'sell'

export function InstrumentInvestPage() {
  const { accessToken: token } = useAuth()
  const [params] = useSearchParams()
  const instrumentId = params.get('instrument')
  const [state, setState] = useState<State>(instrumentId ? { status: 'loading' } : { status: 'empty' })
  const [side, setSide] = useState<OrderSide>('buy')
  const [quantity, setQuantity] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [trade, setTrade] = useState<InstrumentPaperTrade | null>(null)
  const [message, setMessage] = useState<string | null>(null)

  useEffect(() => {
    if (!token || !instrumentId) return
    const controller = new AbortController()
    const headers = { Authorization: `Bearer ${token}` }
    Promise.all([
      apiRequest<PaperAccount[]>('/paper-accounts', { headers, signal: controller.signal }),
      apiRequest<InstrumentDetail>(`/instruments/${instrumentId}?history_limit=120`, { signal: controller.signal }),
    ]).then(async ([accounts, instrument]) => {
      const listed = instrument.instrument_kind === 'listed_security'
      const currency = listed ? instrument.currency : 'USD'
      let account = listed ? accounts.find((item) => item.base_currency === currency) : accounts.at(0)
      if (!account) {
        account = await apiRequest<PaperAccount>('/paper-accounts', {
          method: 'POST', headers,
          body: JSON.stringify({ name: listed ? `${currency} Listed Securities` : 'Global Paper Portfolio', base_currency: currency, starting_cash: '100000' }),
        })
      }
      const performance = await apiRequest<InstrumentPortfolioPerformance>(`/paper-accounts/${account.id}/instrument-performance`, { headers, signal: controller.signal })
      setState({ status: 'ready', instrument, account, performance })
    }).catch((error: unknown) => {
      if (error instanceof DOMException && error.name === 'AbortError') return
      setState({ status: 'error', message: error instanceof Error ? error.message : 'Paper instrument is unavailable.' })
    })
    return () => controller.abort()
  }, [instrumentId, token])

  async function refreshPortfolio(current: ReadyState) {
    if (!token) return
    const headers = { Authorization: `Bearer ${token}` }
    const [account, performance] = await Promise.all([
      apiRequest<PaperAccount>(`/paper-accounts/${current.account.id}`, { headers }),
      apiRequest<InstrumentPortfolioPerformance>(`/paper-accounts/${current.account.id}/instrument-performance`, { headers }),
    ])
    setState({ ...current, account, performance })
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!token || state.status !== 'ready') return
    setSubmitting(true)
    setMessage(null)
    try {
      const payload = side === 'buy'
        ? { instrument_id: state.instrument.id, side, amount: quantity }
        : { instrument_id: state.instrument.id, side, units: quantity }
      const result = await apiRequest<InstrumentPaperTrade>(`/paper-accounts/${state.account.id}/instrument-orders`, {
        method: 'POST', headers: { Authorization: `Bearer ${token}` }, body: JSON.stringify(payload),
      })
      setTrade(result)
      setQuantity('')
      await refreshPortfolio(state)
      setMessage(side === 'buy' ? 'Paper market position opened.' : 'Paper units sold and demo cash settled.')
    } catch (error) {
      setMessage(error instanceof Error ? error.message : 'The paper order could not be completed.')
    } finally {
      setSubmitting(false)
    }
  }

  if (state.status === 'loading') return <main className="lab-state">Preparing the market contract…</main>
  if (state.status === 'error') return <main className="lab-state lab-state--error">{state.message}</main>
  if (state.status === 'empty') return <main className="simulate-empty"><DataLabel>05 / Paper markets</DataLabel><h1>Select a<br />market first.</h1><Link to="/">Open global markets ↗</Link></main>

  const latest = state.instrument.history.at(0)
  const oldest = state.instrument.history.at(-1)
  const listed = state.instrument.instrument_kind === 'listed_security'
  const seriesChange = latest?.annual_change_percent ?? (latest && oldest && latest !== oldest ? String((Number(latest.value) / Number(oldest.value) - 1) * 100) : null)
  const holding = state.performance.positions.find((position) => position.instrument_id === state.instrument.id)
  const exposure = side === 'buy' && quantity ? Number(quantity) / Number(state.account.cash_balance) * 100 : null
  const max = side === 'buy' ? state.account.cash_balance : holding?.units

  return <main className="simulate-page instrument-invest-page">
    <section className="simulate-context">
      <DataLabel>05 / Paper market contract</DataLabel>
      <p>{state.instrument.country_code} / {listed ? 'LISTED REAL ESTATE SECURITY' : 'OFFICIAL RESIDENTIAL INDEX'}</p>
      <h1>{state.instrument.name}</h1>
      <div>
        <span><DataLabel>{listed ? 'Latest EOD close' : 'Latest index mark'}</DataLabel><strong>{latest ? (listed ? formatMoney(latest.value, latest.currency) : `${Number(latest.value).toFixed(2)} pts`) : '—'}</strong></span>
        <span><DataLabel>{latest?.annual_change_percent ? '12M movement' : 'Available-series movement'}</DataLabel><strong>{formatPercent(seriesChange)}</strong></span>
      </div>
    </section>
    <section className="order-ticket">
      <DataLabel>Manage paper position</DataLabel>
      <h2>{holding ? 'Shape the position.' : 'Put capital behind the view.'}</h2>
      <div className="order-side-switch" aria-label="Order side">
        <button type="button" className={side === 'buy' ? 'is-active' : ''} onClick={() => changeSide('buy')}>Buy</button>
        <button type="button" disabled={!holding} className={side === 'sell' ? 'is-active' : ''} onClick={() => changeSide('sell')}>Sell</button>
      </div>
      <dl>
        <div><dt>Available demo cash</dt><dd>{formatMoney(state.account.cash_balance, state.account.base_currency)}</dd></div>
        <div><dt>Units held</dt><dd>{holding ? Number(holding.units).toFixed(6) : '0.000000'}</dd></div>
        <div><dt>Position value</dt><dd>{holding ? formatMoney(holding.market_value, holding.settlement_currency) : '—'}</dd></div>
        <div><dt>Unrealized return</dt><dd className={holding && Number(holding.return_percent) < 0 ? 'signal--negative' : 'signal--positive'}>{holding ? formatPercent(holding.return_percent) : '—'}</dd></div>
        <div><dt>{side === 'buy' ? 'Exposure after order' : 'Latest verified mark'}</dt><dd>{side === 'buy' ? (exposure === null ? '—' : `${exposure.toFixed(2)}% of cash`) : (latest?.observed_on ?? '—')}</dd></div>
      </dl>
      {trade ? <div className="order-confirmation">
        <DataLabel>{trade.side === 'buy' ? 'Position opened' : 'Units sold'}</DataLabel>
        <strong>{formatMoney(trade.gross_amount, trade.settlement_currency)}</strong>
        <span>{Number(trade.units).toFixed(6)} {listed ? 'paper shares' : 'synthetic units'} at {listed ? formatMoney(trade.execution_price, trade.settlement_currency) : `${Number(trade.execution_price).toFixed(2)} index points`}</span>
        <button type="button" onClick={() => setTrade(null)}>Place another order</button>
        <Link to="/portfolio">View portfolio ↗</Link>
      </div> : <form onSubmit={submit}>
        <label>
          <span>{side === 'buy' ? `Virtual investment / ${state.account.base_currency}` : `Units to sell / max ${holding ? Number(holding.units).toFixed(6) : '0'}`}</span>
          <input required min="0.000001" max={max} step="0.000001" inputMode="decimal" value={quantity} onChange={(event) => setQuantity(event.target.value)} placeholder="0.00" />
        </label>
        {side === 'sell' && holding && <button className="order-ticket__all" type="button" onClick={() => setQuantity(holding.units)}>Use full position</button>}
        <button disabled={submitting} type="submit">{submitting ? 'Executing paper order…' : side === 'buy' ? 'Open paper position' : 'Sell paper units'}</button>
      </form>}
      {message && <p className="order-message" role="status">{message}</p>}
      <p className="paper-disclosure">{listed ? 'Development simulation using source-backed end-of-day closes. No security is purchased; the prototype quote feed is not licensed for production redistribution.' : 'Simulation only. The position follows a verified market index. No property, security, fund, or financial instrument is purchased.'}</p>
    </section>
  </main>

  function changeSide(next: OrderSide) {
    setSide(next)
    setTrade(null)
    setMessage(null)
    setQuantity('')
  }
}
