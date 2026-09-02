import { geoCentroid, geoNaturalEarth1, geoPath } from 'd3-geo'
import { feature } from 'topojson-client'
import countriesTopology from 'world-atlas/countries-110m.json'
import type { FeatureCollection, Geometry } from 'geojson'
import type { GeometryCollection, Topology } from 'topojson-specification'

const topology = countriesTopology as unknown as Topology<{ countries: GeometryCollection }>
export const worldCountries = feature(topology, topology.objects.countries) as unknown as FeatureCollection<Geometry>
const projection = geoNaturalEarth1().fitExtent([[18, 18], [982, 482]], worldCountries)
export const worldPath = geoPath(projection)

export function projectMarketPoint(longitude: number, latitude: number) {
  const point = projection([longitude, latitude])
  return point ? { left: `${point[0] / 10}%`, top: `${point[1] / 5}%` } : null
}

const countryAliases: Record<string, string> = {
  'United States': 'United States of America',
  'Hong Kong SAR': 'Hong Kong',
  Korea: 'South Korea',
  Türkiye: 'Turkey',
}

const countryCoordinateFallbacks: Record<string, [number, number]> = {
  Czechia: [15.47, 49.82],
  'Hong Kong SAR': [114.17, 22.32],
  Malta: [14.38, 35.94],
  'North Macedonia': [21.75, 41.61],
  Singapore: [103.82, 1.35],
}

export function projectCountryName(name: string) {
  const target = countryAliases[name] ?? name
  const country = worldCountries.features.find(
    (item) => String(item.properties?.name ?? '').toLowerCase() === target.toLowerCase(),
  )
  if (!country) {
    const fallback = countryCoordinateFallbacks[name]
    return fallback ? projectMarketPoint(fallback[0], fallback[1]) : null
  }
  const [longitude, latitude] = geoCentroid(country)
  return projectMarketPoint(longitude, latitude)
}
