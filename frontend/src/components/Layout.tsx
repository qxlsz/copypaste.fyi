import clsx from 'clsx'
import { NavLink, Outlet } from 'react-router-dom'

import { ThemeToggle } from './ThemeToggle'

const navLinkClass = ({ isActive }: { isActive: boolean }) =>
  clsx(
    'rounded-md px-3 py-2 text-sm font-medium transition-colors text-slate-600 dark:text-slate-300',
    isActive
      ? 'bg-primary/15 text-primary dark:bg-primary/20'
      : 'hover:bg-primary/10 hover:text-primary'
  )

export const Layout = () => (
  <div className="min-h-screen bg-background text-slate-900 transition-colors duration-200 dark:text-slate-100">
    <header className="border-b border-slate-200/80 bg-surface/95 backdrop-blur dark:border-slate-800">
      <div className="mx-auto flex h-16 max-w-6xl items-center justify-between px-6">
        <NavLink to="/" className="text-lg font-semibold text-primary">
          copypaste.fyi
        </NavLink>
        <nav className="flex items-center gap-3">
          <NavLink to="/" className={navLinkClass} end>
            Create Paste
          </NavLink>
          <NavLink to="/stats" className={navLinkClass}>
            Stats
          </NavLink>
          <ThemeToggle />
        </nav>
      </div>
    </header>
    <main className="mx-auto max-w-6xl px-6 py-10">
      <Outlet />
    </main>
  </div>
)
