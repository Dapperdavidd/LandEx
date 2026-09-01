export type InstrumentKind = 'direct_property' | 'listed_security' | 'fractional_offering' | 'market_proxy'
export type InstrumentStatus = 'research' | 'paper_tradeable' | 'real_investible' | 'inactive'

export interface InvestmentInstrument {
  id: string
  slug: string
  name: string
  instrument_kind: InstrumentKind
  status: InstrumentStatus
  country_code: string
  currency: string
  symbol: string | null
  exchange: string | null
  location_id: string | null
  property_id: string | null
  source_url: string | null
  valuation_method: string
  liquidity_class: 'listed' | 'index_proxy' | 'illiquid' | 'unknown'
  real_money_enabled: boolean
  metadata: Record<string, unknown>
  observed_on: string | null
  value: string | null
  annual_change_percent: string | null
  income_yield_percent: string | null
}

export interface InstrumentObservation {
  observed_on: string
  value: string
  currency: string
  annual_change_percent: string | null
  income_yield_percent: string | null
  source_url: string | null
  methodology: string
  metadata: Record<string, unknown>
}

export interface InstrumentPage { items: InvestmentInstrument[]; total: number; limit: number; offset: number }
export interface InstrumentDetail extends Omit<InvestmentInstrument, 'observed_on' | 'value' | 'annual_change_percent' | 'income_yield_percent'> { history: InstrumentObservation[] }

export interface CountryCoverage {
  country_code: string
  country_name: string
  coverage_depth: 'planned' | 'basic' | 'standard' | 'deep'
  has_market_data: boolean
  has_historical_data: boolean
  has_active_listings: boolean
  has_investible_offerings: boolean
  provider_slugs: string[]
  methodology: string | null
  latest_observation_on: string | null
}
