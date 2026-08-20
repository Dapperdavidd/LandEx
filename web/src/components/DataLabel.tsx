import type { ReactNode } from 'react'

export function DataLabel({ children }: { children: ReactNode }) {
  return <span className="data-label">{children}</span>
}
