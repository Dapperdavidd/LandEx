import { DataLabel } from '../components/DataLabel'

export function PlaceholderPage({ section, description }: { section: string; description: string }) {
  return (
    <main className="placeholder-page">
      <DataLabel>LandEX / {section}</DataLabel>
      <h1>{section}</h1>
      <p>{description}</p>
      <span>Interface phase pending</span>
    </main>
  )
}
