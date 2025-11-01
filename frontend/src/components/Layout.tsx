import clsx from 'clsx'
import { NavLink, Outlet } from 'react-router-dom'

const navLinkClass = ({ isActive }: { isActive: boolean }) =>
  clsx(
    'rounded-md px-3 py-2 text-sm font-medium transition-colors',
    isActive
      ? 'bg-primary/10 text-primary'
      : 'text-gray-300 hover:bg-primary/5 hover:text-primary'
  )

export const Layout = () => (
  <div className="min-h-screen bg-background text-gray-100">
    <header className="border-b border-slate-800 bg-surface/80 backdrop-blur">
      <div className="mx-auto flex h-16 max-w-6xl items-center justify-between px-6">
        <NavLink to="/" className="text-lg font-semibold text-primary">
          copypaste.fyi
        </NavLink>
        <nav className="flex items-center gap-2">
          <NavLink to="/" className={navLinkClass} end>
            Create Paste
          </NavLink>
          <NavLink to="/stats" className={navLinkClass}>
            Stats
          </NavLink>
        </nav>
      </div>
    </header>
    <main className="mx-auto max-w-6xl px-6 py-10">
      <Outlet />
    </main>
  </div>
)
