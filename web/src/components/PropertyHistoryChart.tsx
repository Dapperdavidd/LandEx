import type { PropertyHistoryPoint } from '../types/property'

export function PropertyHistoryChart({ points }: { points: PropertyHistoryPoint[] }) {
  const ordered = [...points].reverse()
  const values = ordered.map((point) => Number(point.asking_price ?? point.estimated_value)).filter(Number.isFinite)
  if (values.length < 2) return <div className="history-empty">More observations are needed before a price trend can be drawn.</div>
  const min = Math.min(...values); const max = Math.max(...values); const spread = max - min || 1
  const path = values.map((value, index) => `${index ? 'L' : 'M'} ${(index / (values.length - 1)) * 100} ${94 - ((value - min) / spread) * 82}`).join(' ')
  return <div className="history-chart">
    <svg viewBox="0 0 100 100" preserveAspectRatio="none" role="img" aria-label="Property price history"><path className="history-chart__area" d={`${path} L 100 100 L 0 100 Z`} /><path className="history-chart__line" d={path} /></svg>
    <div><span>{ordered[0]?.observed_on}</span><span>{ordered.at(-1)?.observed_on}</span></div>
  </div>
}
