import { useEffect, useState, type FormEvent } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useAuth } from '../auth/auth-context'
import { DataLabel } from '../components/DataLabel'
import { apiRequest } from '../lib/api'
import { formatMoney, formatPercent } from '../lib/format'
import type { PaperAccount, PaperTrade } from '../types/paper'
import type { PropertyListItem } from '../types/property'

type LabState = { status: 'loading' } | { status: 'empty'; accounts: PaperAccount[] } | { status: 'ready'; property: PropertyListItem; account: PaperAccount } | { status: 'error'; message: string }
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
  return <main className="simulate-page"><section className="simulate-context"><DataLabel>05 / Paper investment</DataLabel><p>{state.property.location_name} / {state.property.country_code}</p><h1>{state.property.address_line || `${state.property.property_type} / ${state.property.location_name}`}</h1><div><span><DataLabel>Asset price</DataLabel><strong>{formatMoney(state.property.price, state.property.currency)}</strong></span><span><DataLabel>Yield</DataLabel><strong>{formatPercent(state.property.gross_yield_percent)}</strong></span></div></section><section className="order-ticket"><DataLabel>Open position</DataLabel><h2>Put capital<br />behind the view.</h2><dl><div><dt>Available demo cash</dt><dd>{formatMoney(state.account.cash_balance, state.account.base_currency)}</dd></div><div><dt>Account</dt><dd>{state.account.name}</dd></div><div><dt>Exposure after order</dt><dd>{exposure === null ? '—' : `${exposure.toFixed(2)}%`}</dd></div></dl>{trade ? <div className="order-confirmation"><DataLabel>Position opened</DataLabel><strong>{formatMoney(trade.gross_amount, trade.currency)}</strong><span>{trade.units} units at {formatMoney(trade.execution_price, trade.currency)}</span><Link to="/portfolio">View portfolio ↗</Link></div> : <form onSubmit={submit}><label><span>Virtual investment / {state.account.base_currency}</span><input required min="0.01" max={state.account.cash_balance} step="0.01" inputMode="decimal" value={amount} onChange={(event) => setAmount(event.target.value)} placeholder="0.00" /></label><button disabled={submitting} type="submit">{submitting ? 'Opening position…' : 'Open position'}</button></form>}{message && <p className="order-message" role="status">{message}</p>}<p className="paper-disclosure">Simulation only. No property, security, or financial instrument is purchased.</p></section></main>
}
function LabState({ children, error = false }: { children: string; error?: boolean }) { return <main className={`lab-state${error ? ' lab-state--error' : ''}`}>{children}</main> }
