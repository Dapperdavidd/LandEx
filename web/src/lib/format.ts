export function formatMoney(value: string | null, currency: string | null): string {
  if (value === null || currency === null) return '—'
  const number = Number(value)
  if (!Number.isFinite(number)) return '—'
  return new Intl.NumberFormat('en', {
    style: 'currency',
    currency,
    notation: number >= 1_000_000 ? 'compact' : 'standard',
    maximumFractionDigits: number >= 1_000 ? 0 : 2,
  }).format(number)
}

export function formatPercent(value: string | null): string {
  if (value === null || !Number.isFinite(Number(value))) return '—'
  const number = Number(value)
  return `${number > 0 ? '+' : ''}${number.toFixed(2)}%`
}
