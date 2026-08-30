import { geoNaturalEarth1, geoPath } from 'd3-geo'
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
