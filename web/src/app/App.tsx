import { Route, Routes } from 'react-router-dom'
import { TerminalShell } from '../components/TerminalShell'
import { MarketsPage } from '../pages/MarketsPage'
import { PlaceholderPage } from '../pages/PlaceholderPage'

export function App() {
  return (
    <TerminalShell>
      <Routes>
        <Route path="/" element={<MarketsPage />} />
        <Route path="/explore" element={<PlaceholderPage section="Explore" description="Research real properties through price, yield, growth and place." />} />
        <Route path="/watchlist" element={<PlaceholderPage section="Watchlist" description="Markets and properties worth watching, reduced to signal." />} />
        <Route path="/portfolio" element={<PlaceholderPage section="Portfolio" description="Your simulated exposure to the physical world." />} />
        <Route path="/simulate" element={<PlaceholderPage section="Simulate" description="Model how capital could move through markets over time." />} />
        <Route path="*" element={<PlaceholderPage section="Not found" description="That market surface does not exist." />} />
      </Routes>
    </TerminalShell>
  )
}
