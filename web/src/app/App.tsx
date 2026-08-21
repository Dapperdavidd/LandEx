import { Route, Routes } from 'react-router-dom'
import { TerminalShell } from '../components/TerminalShell'
import { MarketsPage } from '../pages/MarketsPage'
import { PlaceholderPage } from '../pages/PlaceholderPage'
import { AccessPage } from '../pages/AccessPage'
import { ProfilePage } from '../pages/ProfilePage'
import { ProtectedRoute } from '../auth/ProtectedRoute'
import { ExplorePage } from '../pages/ExplorePage'
import { PropertyPage } from '../pages/PropertyPage'
import { WatchlistPage } from '../pages/WatchlistPage'

export function App() {
  return (
    <TerminalShell>
      <Routes>
        <Route path="/" element={<MarketsPage />} />
        <Route path="/explore" element={<ExplorePage />} />
        <Route path="/properties/:id" element={<PropertyPage />} />
        <Route path="/access" element={<AccessPage />} />
        <Route path="/watchlist" element={<ProtectedRoute><WatchlistPage /></ProtectedRoute>} />
        <Route path="/portfolio" element={<ProtectedRoute><PlaceholderPage section="Portfolio" description="Your simulated exposure to the physical world." /></ProtectedRoute>} />
        <Route path="/simulate" element={<ProtectedRoute><PlaceholderPage section="Simulate" description="Model how capital could move through markets over time." /></ProtectedRoute>} />
        <Route path="/profile" element={<ProtectedRoute><ProfilePage /></ProtectedRoute>} />
        <Route path="*" element={<PlaceholderPage section="Not found" description="That market surface does not exist." />} />
      </Routes>
    </TerminalShell>
  )
}
