import { useMemo, useState } from 'react'
import clsx from 'clsx'
import { NavLink, Outlet, useLocation, useNavigate } from 'react-router-dom'

import { ThemeToggle } from './ThemeToggle'
import { Button } from './ui/Button'
import { CommandPalette } from './CommandPalette'
import { useHotkeys } from '../hooks/useHotkeys'

const navLinkClass = ({ isActive }: { isActive: boolean }) =>
  clsx(
    'rounded-md px-3 py-2 text-sm font-medium transition-colors text-slate-600 dark:text-slate-300',
    isActive
      ? 'bg-primary/15 text-primary dark:bg-primary/20'
      : 'hover:bg-primary/10 hover:text-primary'
  )

export const Layout = () => {
  const navigate = useNavigate()
  const [isPaletteOpen, setPaletteOpen] = useState(false)
  const location = useLocation()
  const showHero = location.pathname === '/'

  const commandActions = useMemo(
    () => [
      {
        id: 'create-paste',
        label: 'Create new paste',
        description: 'Jump straight to the composer with default retention and encryption.',
        shortcut: '⌘N',
        group: 'Primary',
        handler: () => navigate('/'),
      },
      {
        id: 'dashboard',
        label: 'Open dashboard',
        description: 'Review key metrics, quick actions, and recent workspace activity.',
        group: 'Navigation',
        handler: () => navigate('/dashboard'),
      },
      {
        id: 'view-stats',
        label: 'View stats',
        description: 'Deep-dive into format, encryption, and retention analytics.',
        group: 'Navigation',
        handler: () => navigate('/stats'),
      },
    ],
    [navigate]
  )

  useHotkeys({ shortcut: 'meta+n', handler: () => navigate('/') })
  useHotkeys({ shortcut: 'ctrl+n', handler: () => navigate('/') })

  return (
    <div className="min-h-screen bg-background text-slate-900 transition-colors duration-200 dark:text-slate-100">
      <CommandPalette actions={commandActions} isOpen={isPaletteOpen} onOpenChange={setPaletteOpen} />
      <header className="border-b border-slate-200/80 bg-surface/95 backdrop-blur dark:border-slate-800">
        <div className="mx-auto flex max-w-6xl flex-col gap-3 px-6 py-4">
          <div className="flex flex-col items-start justify-between gap-2 md:flex-row md:items-center">
            <NavLink to="/" className="text-lg font-semibold text-primary">
              copypaste.fyi
            </NavLink>
            <nav className="flex flex-wrap items-center gap-2 md:gap-3">
              <Button variant="ghost" size="sm" className="hidden md:inline-flex" onClick={() => navigate('/')}> 
                New paste (⌘N)
              </Button>
              <NavLink to="/" className={navLinkClass} end>
                Create Paste
              </NavLink>
              <NavLink to="/dashboard" className={navLinkClass}>
                Dashboard
              </NavLink>
              <NavLink to="/stats" className={navLinkClass}>
                Stats
              </NavLink>
              <Button variant="ghost" size="sm" onClick={() => setPaletteOpen(true)}>
                Command Menu
              </Button>
              <ThemeToggle />
            </nav>
          </div>
          {showHero && (
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Secure paste — encrypt, time-limit, or burn after reading. Your keys stay in the browser.
            </p>
          )}
        </div>
      </header>
      <main className="mx-auto max-w-6xl px-6 py-6">
        <Outlet />
      </main>
    </div>
  )
}
