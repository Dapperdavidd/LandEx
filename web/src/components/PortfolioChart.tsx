import type { PortfolioSnapshot } from '../types/paper'
export function PortfolioChart({ snapshots }: { snapshots: PortfolioSnapshot[] }) {
  const points = [...snapshots].reverse(); const values = points.map((point) => Number(point.total_value)).filter(Number.isFinite)
  if (values.length < 2) return <div className="portfolio-chart-empty">Daily portfolio history will appear after the next valuation snapshots.</div>
  const min = Math.min(...values); const max = Math.max(...values); const spread = max - min || 1
  const path = values.map((value, index) => `${index ? 'L' : 'M'} ${(index / (values.length - 1)) * 100} ${94 - ((value - min) / spread) * 82}`).join(' ')
  return <div className="portfolio-chart"><svg viewBox="0 0 100 100" preserveAspectRatio="none" role="img" aria-label="Portfolio value history"><path d={`${path} L 100 100 L 0 100 Z`} /><path d={path} /></svg><div><span>{points[0]?.observed_on}</span><span>{points.at(-1)?.observed_on}</span></div></div>
}
