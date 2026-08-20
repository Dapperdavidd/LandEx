export interface LatestMarketMetric {
  observed_on: string | null
  currency: string | null
  median_sale_price: string | null
  median_rent_monthly: string | null
  gross_yield_percent: string | null
  annual_growth_percent: string | null
  active_inventory: number | null
  days_on_market: string | null
}

export interface MarketSummary {
  id: string
  name: string
  property_type: string | null
  location_id: string
  location_name: string
  location_kind: string
  country_code: string
  latest: LatestMarketMetric
}

export interface MarketPage {
  items: MarketSummary[]
  total: number
  limit: number
  offset: number
}
