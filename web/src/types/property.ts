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
export interface PropertyHistoryPoint { observed_on: string; asking_price: string | null; rental_price_monthly: string | null; shortlet_price_nightly: string | null; estimated_value: string | null; currency: string; days_on_market: number | null }
export interface ScoreComponent { name: string; score: string | null; methodology: string }
export interface PropertyScore { overall_score: string | null; components: ScoreComponent[]; unavailable_components: string[] }
export interface LocationCategory { category: string; feature_count: number; nearest_distance_meters: number }
export interface NearbyFeature { id: string; category: string; kind: string; name: string | null; latitude: number; longitude: number; distance_meters: number; observed_at: string; expires_at: string }
export interface PropertyLocationIntelligence { property_id: string; property_latitude: number | null; property_longitude: number | null; radius_meters: number; cache: { populated: boolean; fresh: boolean; observed_at: string | null; expires_at: string | null }; categories: LocationCategory[]; features: NearbyFeature[] }
export interface WatchlistSummary { id: string; name: string; item_count: number; created_at: string; updated_at: string }
export interface WatchlistItem { id: string; property_id: string | null; market_id: string | null; location_id: string | null; created_at: string }
export interface WatchlistDetail extends WatchlistSummary { items: WatchlistItem[] }
