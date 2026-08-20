import { DataLabel } from './DataLabel'

interface MarketNumberProps {
  label: string
  value: string
  signal?: 'positive' | 'negative' | 'neutral'
  compact?: boolean
}

export function MarketNumber({ label, value, signal = 'neutral', compact }: MarketNumberProps) {
  return (
    <div className={compact ? 'market-number market-number--compact' : 'market-number'}>
      <DataLabel>{label}</DataLabel>
      <strong className={`market-number__value signal--${signal}`}>{value}</strong>
    </div>
  )
}
