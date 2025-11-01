import clsx from 'clsx'
import type { ReactElement } from 'react'
import { useMemo } from 'react'

import { useTheme } from '../hooks/useTheme'
import type { Theme } from '../theme/ThemeContext'

const icons: Record<Theme, ReactElement> = {
  light: (
    <svg viewBox="0 0 24 24" className="h-4 w-4" aria-hidden>
      <path fill="currentColor" d="M12 18a1 1 0 0 1 1 1v2a1 1 0 1 1-2 0v-2a1 1 0 0 1 1-1Zm7.071-3.071a1 1 0 0 1 1.414 1.414l-1.415 1.414a1 1 0 0 1-1.414-1.414ZM12 6a1 1 0 0 1-1-1V3a1 1 0 0 1 2 0v2a1 1 0 0 1-1 1Zm6.364-3.536a1 1 0 0 1 1.415 1.414L18.364 5.293a1 1 0 0 1-1.414-1.414ZM5 12a1 1 0 0 1-1 1H2a1 1 0 1 1 0-2h2a1 1 0 0 1 1 1Zm17 0a1 1 0 0 1-1 1h-2a1 1 0 1 1 0-2h2a1 1 0 0 1 1 1ZM5.636 3.636a1 1 0 0 1 1.414-1.414L8.465 3.636A1 1 0 0 1 7.05 5.05ZM6 12a6 6 0 1 1 6 6 6 6 0 0 1-6-6Zm6 4a4 4 0 1 0-4-4 4 4 0 0 0 4 4Zm-6.364.707a1 1 0 0 1 0 1.414L4.222 19.12a1 1 0 0 1-1.414-1.414l1.414-1.415a1 1 0 0 1 1.414 0Z" />
    </svg>
  ),
  dark: (
    <svg viewBox="0 0 24 24" className="h-4 w-4" aria-hidden>
      <path
        fill="currentColor"
        d="M21 12.79A9 9 0 0 1 11.21 3a7 7 0 1 0 9.79 9.79ZM12 22a10 10 0 0 1-1.77-19.85 1 1 0 0 1 1.14 1.45A7 7 0 1 0 20.4 12.63a1 1 0 0 1 1.45 1.13A10 10 0 0 1 12 22Z"
      />
    </svg>
  ),
}

export const ThemeToggle = () => {
  const { theme, toggleTheme } = useTheme()
  const { icon, label } = useMemo(
    () => ({
      icon: icons[theme],
      label: theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode',
    }),
    [theme]
  )

  return (
    <button
      type="button"
      onClick={toggleTheme}
      className={clsx(
        'inline-flex h-9 items-center gap-2 rounded-full border border-slate-300 bg-surface px-3 text-xs font-medium shadow-sm transition',
        'hover:border-primary hover:text-primary focus:outline-none focus:ring focus:ring-primary/30 dark:border-slate-700 dark:hover:border-accent'
      )}
      aria-label={label}
      title={label}
    >
      {icon}
      <span className="capitalize text-slate-700 dark:text-slate-200">{theme}</span>
    </button>
  )
}
