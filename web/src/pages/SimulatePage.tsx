import { useEffect, useState, type FormEvent } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useAuth } from '../auth/auth-context'
import { DataLabel } from '../components/DataLabel'
import { apiRequest } from '../lib/api'
import { formatMoney, formatPercent } from '../lib/format'
import type { PaperAccount, PaperTrade } from '../types/paper'
import type { PropertyListItem } from '../types/property'

type LabState = { status: 'loading' } | { status: 'empty'; accounts: PaperAccount[] } | { status: 'ready'; property: PropertyListItem; account: PaperAccount } | { status: 'error'; message: string }
type Scenario = { name: string; annual_appreciation_percent: string; projected_property_value: string; projected_cumulative_cash_flow: string; projected_total_profit: string; projected_total_return_percent: string; timeline: { year: number; projected_property_value: string; total_return_percent_if_sold: string }[] }
type ScenarioResult = { holding_years: number; scenarios: Scenario[] }
export function SimulatePage() {
  const auth = useAuth(); const token = auth.accessToken; const [params] = useSearchParams(); const propertyId = params.get('property')
  const [state, setState] = useState<LabState>({ status: 'loading' }); const [amount, setAmount] = useState(''); const [submitting, setSubmitting] = useState(false); const [trade, setTrade] = useState<PaperTrade | null>(null); const [message, setMessage] = useState<string | null>(null)
  useEffect(() => {
    if (!token) return
    const controller = new AbortController(); const headers = { Authorization: `Bearer ${token}` }
    Promise.all([apiRequest<PaperAccount[]>('/paper-accounts', { headers, signal: controller.signal }), propertyId ? apiRequest<PropertyListItem>(`/properties/${propertyId}`, { signal: controller.signal }) : Promise.resolve(null)]).then(async ([accounts, property]) => {
      if (!property) { setState({ status: 'empty', accounts }); return }
      let account = accounts.find((candidate) => candidate.base_currency === property.currency)
      if (!account) account = await apiRequest<PaperAccount>('/paper-accounts', { method: 'POST', headers, body: JSON.stringify({ name: `Demo ${property.currency}`, base_currency: property.currency, starting_cash: '100000' }) })
      setState({ status: 'ready', property, account })
    }).catch((error: unknown) => { if (error instanceof DOMException && error.name === 'AbortError') return; setState({ status: 'error', message: error instanceof Error ? error.message : 'Simulation data is unavailable.' }) })
    return () => controller.abort()
  }, [token, propertyId])
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (!token || state.status !== 'ready') return; setSubmitting(true); setMessage(null)
    try { const result = await apiRequest<PaperTrade>(`/paper-accounts/${state.account.id}/orders`, { method: 'POST', headers: { Authorization: `Bearer ${token}` }, body: JSON.stringify({ property_id: state.property.id, side: 'buy', amount }) }); setTrade(result); setState({ ...state, account: { ...state.account, cash_balance: String(Number(state.account.cash_balance) - Number(result.gross_amount)) } }); setMessage('Position opened. Your portfolio now tracks this asset.') }
    catch (error) { setMessage(error instanceof Error ? error.message : 'The paper order could not be completed.') } finally { setSubmitting(false) }
  }
  if (state.status === 'loading') return <LabState>Preparing the investment lab…</LabState>
  if (state.status === 'error') return <LabState error>{state.message}</LabState>
  if (state.status === 'empty') return <main className="simulate-empty"><DataLabel>05 / Investment lab</DataLabel><h1>Select an<br />asset first.</h1><p>Open a property from Explore and choose Paper Invest to construct a position from real normalized pricing.</p><Link to="/explore">Explore properties ↗</Link></main>
  const exposure = amount && Number(state.account.cash_balance) > 0 ? Number(amount) / Number(state.account.cash_balance) * 100 : null
  return <main className="simulate-page"><section className="simulate-context"><DataLabel>05 / Paper investment</DataLabel><p>{state.property.location_name} / {state.property.country_code}</p><h1>{state.property.address_line || `${state.property.property_type} / ${state.property.location_name}`}</h1><div><span><DataLabel>Asset price</DataLabel><strong>{formatMoney(state.property.price, state.property.currency)}</strong></span><span><DataLabel>Yield</DataLabel><strong>{formatPercent(state.property.gross_yield_percent)}</strong></span></div></section><section className="order-ticket"><DataLabel>Open position</DataLabel><h2>Put capital<br />behind the view.</h2><dl><div><dt>Available demo cash</dt><dd>{formatMoney(state.account.cash_balance, state.account.base_currency)}</dd></div><div><dt>Account</dt><dd>{state.account.name}</dd></div><div><dt>Exposure after order</dt><dd>{exposure === null ? '—' : `${exposure.toFixed(2)}%`}</dd></div></dl>{trade ? <div className="order-confirmation"><DataLabel>Position opened</DataLabel><strong>{formatMoney(trade.gross_amount, trade.currency)}</strong><span>{trade.units} units at {formatMoney(trade.execution_price, trade.currency)}</span><Link to="/portfolio">View portfolio ↗</Link></div> : <form onSubmit={submit}><label><span>Virtual investment / {state.account.base_currency}</span><input required min="0.01" max={state.account.cash_balance} step="0.01" inputMode="decimal" value={amount} onChange={(event) => setAmount(event.target.value)} placeholder="0.00" /></label><button disabled={submitting} type="submit">{submitting ? 'Opening position…' : 'Open position'}</button></form>}{message && <p className="order-message" role="status">{message}</p>}<p className="paper-disclosure">Simulation only. No property, security, or financial instrument is purchased.</p></section><ScenarioLab property={state.property} /></main>
}
function ScenarioLab({ property }: { property: PropertyListItem }) {
  const inferredRent = property.gross_yield_percent ? (Number(property.price) * Number(property.gross_yield_percent) / 1200).toFixed(2) : '0'
  const [years, setYears] = useState('10'); const [rent, setRent] = useState(inferredRent); const [base, setBase] = useState('8'); const [result, setResult] = useState<ScenarioResult | null>(null); const [message, setMessage] = useState<string | null>(null); const [running, setRunning] = useState(false)
  async function simulate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setRunning(true); setMessage(null)
    try {
      const response = await apiRequest<ScenarioResult>('/analysis/scenarios', { method: 'POST', body: JSON.stringify({ investment: { purchase_price: property.price, monthly_rent: rent, vacancy_rate_percent: '5', annual_property_tax: '0', annual_insurance: '0', annual_maintenance: '0', annual_management: '0', closing_costs: '0', annual_appreciation_percent: '0', holding_years: Number(years), sale_cost_percent: '6', loan_balance_at_sale: '0', down_payment: null }, conservative_appreciation_percent: String(Number(base) - 4), base_appreciation_percent: base, bull_appreciation_percent: String(Number(base) + 5) }) })
      setResult(response)
    } catch (error) { setMessage(error instanceof Error ? error.message : 'Scenario analysis is unavailable.') } finally { setRunning(false) }
  }
  return <section className="scenario-lab"><header><div><DataLabel>05B / Scenario lab</DataLabel><h2>Test the<br />future.</h2></div><p>Projections are calculated by the LandEX investment engine from your stated assumptions. They are not forecasts or investment advice.</p></header><form onSubmit={simulate}><label><span>Holding years</span><input required min="1" max="100" type="number" value={years} onChange={(event) => setYears(event.target.value)} /></label><label><span>Monthly rent / {property.currency}</span><input required min="0" step="0.01" inputMode="decimal" value={rent} onChange={(event) => setRent(event.target.value)} /></label><label><span>Base appreciation %</span><input required min="-95" max="99" step="0.1" inputMode="decimal" value={base} onChange={(event) => setBase(event.target.value)} /></label><button disabled={running} type="submit">{running ? 'Running…' : 'Run scenarios'}</button></form>{message && <p className="scenario-message">{message}</p>}{result && <div className="scenario-results">{result.scenarios.map((scenario) => <article key={scenario.name}><DataLabel>{scenario.name} / {scenario.annual_appreciation_percent}% annual</DataLabel><strong>{formatMoney(scenario.projected_property_value, property.currency)}</strong><span>Projected property value at year {result.holding_years}</span><dl><div><dt>Total return</dt><dd>{formatPercent(scenario.projected_total_return_percent)}</dd></div><div><dt>Cash flow</dt><dd>{formatMoney(scenario.projected_cumulative_cash_flow, property.currency)}</dd></div><div><dt>Total profit</dt><dd>{formatMoney(scenario.projected_total_profit, property.currency)}</dd></div></dl></article>)}</div>}</section>
}
function LabState({ children, error = false }: { children: string; error?: boolean }) { return <main className={`lab-state${error ? ' lab-state--error' : ''}`}>{children}</main> }
