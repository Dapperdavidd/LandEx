import type { ReactNode } from 'react'
import { NavLink } from 'react-router-dom'

const links = [
  ['/', 'Markets', '01'],
  ['/explore', 'Explore', '02'],
  ['/watchlist', 'Watchlist', '03'],
  ['/portfolio', 'Portfolio', '04'],
  ['/simulate', 'Simulate', '05'],
] as const

export function TerminalShell({ children }: { children: ReactNode }) {
  return (
    <div className="terminal-shell">
      <aside className="nav-rail" aria-label="Primary navigation">
        <NavLink className="brand" to="/" aria-label="LandEX home">
          L<span>X</span>
        </NavLink>
        <nav className="nav-rail__links">
          {links.map(([to, label, index]) => (
            <NavLink key={to} to={to} className={({ isActive }) => `nav-link${isActive ? ' is-active' : ''}`}>
              <span>{index}</span>
              {label}
            </NavLink>
          ))}
        </nav>
        <button className="profile-trigger" type="button" aria-label="Open profile">
          DD
        </button>
      </aside>
      <div className="terminal-main">
        <header className="top-line">
          <span>Global real-estate intelligence</span>
          <button className="command-trigger" type="button">
            <span>Ask the market</span>
            <kbd>⌘ K</kbd>
          </button>
          <time dateTime={new Date().toISOString()}>Live data / UTC</time>
        </header>
        {children}
      </div>
    </div>
  )
}
