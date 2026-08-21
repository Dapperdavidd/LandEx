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
import { SimulatePage } from '../pages/SimulatePage'
import { PortfolioPage } from '../pages/PortfolioPage'
import { MarketDetailPage } from '../pages/MarketDetailPage'
import { MarketComparisonPage } from '../pages/MarketComparisonPage'
import { SignalsPage } from '../pages/SignalsPage'

export function App() {
  return (
    <TerminalShell>
      <Routes>
        <Route path="/" element={<MarketsPage />} />
        <Route path="/markets/:id" element={<MarketDetailPage />} />
        <Route path="/compare" element={<MarketComparisonPage />} />
        <Route path="/signals" element={<ProtectedRoute><SignalsPage /></ProtectedRoute>} />
        <Route path="/explore" element={<ExplorePage />} />
        <Route path="/properties/:id" element={<PropertyPage />} />
        <Route path="/access" element={<AccessPage />} />
        <Route path="/watchlist" element={<ProtectedRoute><WatchlistPage /></ProtectedRoute>} />
        <Route path="/portfolio" element={<ProtectedRoute><PortfolioPage /></ProtectedRoute>} />
        <Route path="/simulate" element={<ProtectedRoute><SimulatePage /></ProtectedRoute>} />
        <Route path="/profile" element={<ProtectedRoute><ProfilePage /></ProtectedRoute>} />
        <Route path="*" element={<PlaceholderPage section="Not found" description="That market surface does not exist." />} />
      </Routes>
    </TerminalShell>
  )
}
