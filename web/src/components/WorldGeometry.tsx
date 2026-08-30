import { worldCountries, worldPath } from '../lib/world-geometry'

export function WorldGeometry() {
  return <svg className="world-geometry" viewBox="0 0 1000 500" preserveAspectRatio="none" aria-hidden="true">
    <defs><filter id="world-glow"><feGaussianBlur stdDeviation="2.2" /></filter></defs>
    <g className="world-geometry__glow" filter="url(#world-glow)">{worldCountries.features.map((country, index) => <path d={worldPath(country) ?? undefined} key={String(country.id ?? index)} />)}</g>
    <g className="world-geometry__land">{worldCountries.features.map((country, index) => <path d={worldPath(country) ?? undefined} key={String(country.id ?? index)} />)}</g>
    <path className="world-geometry__sphere" d={worldPath({ type: 'Sphere' }) ?? undefined} />
  </svg>
}
