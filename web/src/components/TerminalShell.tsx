import { useEffect, useState, type ReactNode } from 'react'
import { NavLink, useNavigate } from 'react-router-dom'
import { useAuth } from '../auth/auth-context'

const links = [
  ['/', 'Markets', '01'],
  ['/explore', 'Explore', '02'],
  ['/watchlist', 'Watchlist', '03'],
  ['/portfolio', 'Portfolio', '04'],
  ['/simulate', 'Simulate', '05'],
] as const

export function TerminalShell({ children }: { children: ReactNode }) {
  const auth = useAuth()
  const navigate = useNavigate()
  const [commandOpen, setCommandOpen] = useState(false)
  const [query, setQuery] = useState('')
  const initials = auth.user?.display_name.split(/\s+/).slice(0, 2).map((part) => part[0]).join('').toUpperCase() ?? 'IN'
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); setCommandOpen(true) }
      if (event.key === 'Escape') setCommandOpen(false)
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [])
  const commands = [
    { label: 'Explore all opportunities', hint: 'Global inventory', to: '/explore' },
    { label: 'Find high-yield opportunities', hint: 'Yield ≥ 7%', to: '/explore?min_yield_percent=7&limit=20' },
    { label: 'Compare markets', hint: 'Research terminal', to: '/compare' },
    { label: 'Open my watchlist', hint: 'Saved assets', to: '/watchlist' },
    { label: 'Review portfolio', hint: 'Paper positions', to: '/portfolio' },
    { label: 'Manage market signals', hint: 'Alerts and saved research', to: '/signals' },
  ].filter((command) => `${command.label} ${command.hint}`.toLowerCase().includes(query.toLowerCase()))
  function run(to: string) { setCommandOpen(false); setQuery(''); navigate(to) }
  return (
    <div className="terminal-shell">
      <a className="skip-link" href="#terminal-content">Skip to market content</a>
      <aside className="nav-rail" aria-label="Primary navigation">
        <NavLink className="brand" to="/" aria-label="LandEX home">
          LANDE<span>X</span>
        </NavLink>
        <nav className="nav-rail__links">
          {links.map(([to, label, index]) => (
            <NavLink key={to} to={to} className={({ isActive }) => `nav-link${isActive ? ' is-active' : ''}`}>
              <span>{index}</span>
              {label}
            </NavLink>
          ))}
        </nav>
        <NavLink className="profile-trigger" to={auth.user ? '/profile' : '/access'} aria-label={auth.user ? 'Open profile' : 'Sign in'}>{initials}</NavLink>
      </aside>
      <div className="terminal-main" id="terminal-content" tabIndex={-1}>
        <header className="top-line">
          <span>Global real-estate intelligence</span>
          <button className="command-trigger" type="button" onClick={() => setCommandOpen(true)}>
            <span>Ask the market</span>
            <kbd>⌘ K</kbd>
          </button>
          <time dateTime={new Date().toISOString()}>Live data / UTC</time>
        </header>
        {children}
      </div>
      {commandOpen && <div className="command-layer" role="dialog" aria-modal="true" aria-label="Ask the market" onMouseDown={() => setCommandOpen(false)}><div className="command-palette" onMouseDown={(event) => event.stopPropagation()}><label><span>Ask the market</span><input autoFocus value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search a market action…" /></label><div>{commands.length ? commands.map((command, index) => <button type="button" key={command.to} onClick={() => run(command.to)}><span>{String(index + 1).padStart(2, '0')}</span><strong>{command.label}</strong><small>{command.hint}</small></button>) : <p>No terminal action matches that query.</p>}</div><footer><span>⌘ K / Ctrl K</span><span>Esc to close</span></footer></div></div>}
    </div>
  )
}
