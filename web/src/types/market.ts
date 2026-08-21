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
  latitude: number | null
  longitude: number | null
  latest: LatestMarketMetric
}

export interface MarketPage {
  items: MarketSummary[]
  total: number
  limit: number
  offset: number
}
export interface MarketMetric { observed_on: string; currency: string; median_sale_price: string | null; median_rent_monthly: string | null; gross_yield_percent: string | null; annual_growth_percent: string | null; active_inventory: number | null; days_on_market: string | null }
export interface MarketDetail { id: string; name: string; property_type: string | null; location_id: string; location_name: string; location_kind: string; country_code: string; history: MarketMetric[] }
export interface MarketComparison { id: string; name: string; property_type: string | null; country_code: string; source_currency: string | null; target_currency: string; observed_on: string | null; conversion_status: string; conversion_rate_date: string | null; median_sale_price: string | null; median_rent_monthly: string | null; gross_yield_percent: string | null; annual_growth_percent: string | null; active_inventory: number | null; days_on_market: string | null }
