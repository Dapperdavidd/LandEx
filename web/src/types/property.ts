export interface PropertyListItem {
  id: string
  property_type: string
  address_line: string | null
  postal_code: string | null
  latitude: number | null
  longitude: number | null
  bedrooms: string | null
  bathrooms: string | null
  area_sqm: string | null
  year_built: number | null
  location_id: string
  location_name: string
  country_code: string
  listing_id: string
  listing_type: string
  listing_status: string
  price: string
  currency: string
  price_period: string
  source_url: string | null
  last_seen_at: string
  gross_yield_percent: string | null
  annual_growth_percent: string | null
  overall_score: string | null
}
export interface PropertyPage { items: PropertyListItem[]; total: number; limit: number; offset: number }
export interface SavedSearch { id: string; name: string; criteria: Record<string, unknown>; created_at: string; updated_at: string }
