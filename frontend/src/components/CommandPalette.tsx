import { useEffect, useMemo, useState } from 'react'
import { createPortal } from 'react-dom'

import { useHotkeys } from '../hooks/useHotkeys.ts'
import type { CommandPaletteAction } from '../types/commandPalette'
import { Button } from './ui/Button'

interface CommandPaletteProps {
  actions: CommandPaletteAction[]
  isOpen?: boolean
  onOpenChange?: (open: boolean) => void
}

interface CommandPaletteOverlayProps {
  isOpen: boolean
  onClose: () => void
  query: string
  onQueryChange: (value: string) => void
  actions: CommandPaletteAction[]
  onSelect: (action: CommandPaletteAction) => void
}

const CommandPaletteOverlay = ({ isOpen, onClose, query, onQueryChange, actions, onSelect }: CommandPaletteOverlayProps) => {
  const [mounted, setMounted] = useState(false)

  useEffect(() => {
    setMounted(true)
    return () => setMounted(false)
  }, [])

  if (!isOpen || !mounted) return null

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-slate-950/60 px-4 pt-[15vh] backdrop-blur">
      <div className="w-full max-w-2xl rounded-2xl border border-muted/60 bg-surface/95 p-6 shadow-strong">
        <div className="space-y-3">
          <div className="flex items-center justify-between text-xs uppercase tracking-wide text-muted-foreground">
            <span>Command palette</span>
            <span className="font-medium text-primary">⌘K</span>
          </div>
          <input
            autoFocus
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder="Type a command or search..."
            className="w-full rounded-xl border border-muted bg-background/90 px-4 py-3 text-base shadow-inner focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/30"
          />
          <div className="space-y-3">
            {actions.length ? (
              Object.entries(
                actions.reduce<Record<string, CommandPaletteAction[]>>((grouped, action) => {
                  const group = action.group ?? 'General'
                  if (!grouped[group]) {
                    grouped[group] = []
                  }
                  grouped[group].push(action)
                  return grouped
                }, {})
              ).map(([group, groupActions]) => (
                <div key={group} className="space-y-2">
                  <p className="text-xs uppercase tracking-wide text-muted-foreground">{group}</p>
                  <div className="grid gap-1">
                    {groupActions.map((action) => (
                      <button
                        key={action.id}
                        type="button"
                        onClick={() => onSelect(action)}
                        className="flex items-center justify-between rounded-xl border border-transparent px-4 py-2 text-left text-sm font-medium text-slate-700 transition hover:border-primary/40 hover:bg-primary/5 hover:text-primary dark:text-slate-200"
                      >
                        <span>
                          {action.label}
                          {action.description ? (
                            <span className="block text-xs font-normal text-muted-foreground">{action.description}</span>
                          ) : null}
                        </span>
                        {action.shortcut ? <span className="text-xs text-muted-foreground">{action.shortcut}</span> : null}
                      </button>
                    ))}
                  </div>
                </div>
              ))
            ) : (
              <div className="rounded-xl border border-dashed border-muted px-4 py-10 text-center text-sm text-muted-foreground">
                No commands found{query ? ` for "${query}"` : ''}.
              </div>
            )}
          </div>
          <div className="flex justify-end">
            <Button variant="ghost" size="sm" onClick={onClose}>
              Close
            </Button>
          </div>
        </div>
      </div>
    </div>,
    document.body
  )
}

export const CommandPalette = ({ actions, isOpen: controlledOpen, onOpenChange }: CommandPaletteProps) => {
  const [internalOpen, setInternalOpen] = useState(false)
  const [query, setQuery] = useState('')

  const isControlled = typeof controlledOpen === 'boolean'
  const isOpen = isControlled ? controlledOpen : internalOpen

  const setOpen = (value: boolean) => {
    if (!isControlled) {
      setInternalOpen(value)
    }
    if (!value) {
      setQuery('')
    }
    onOpenChange?.(value)
  }

  const sortedActions = useMemo(
    () => actions.slice().sort((a, b) => a.label.localeCompare(b.label)),
    [actions]
  )

  const filteredActions = useMemo(() => {
    if (!query.trim()) {
      return sortedActions
    }
    const term = query.toLowerCase()
    return sortedActions.filter((action) => action.label.toLowerCase().includes(term))
  }, [sortedActions, query])

  useHotkeys({
    shortcut: 'meta+k',
    handler: () => setOpen(!isOpen),
  })

  useHotkeys({
    shortcut: 'ctrl+k',
    handler: () => setOpen(!isOpen),
  })

  useHotkeys({
    shortcut: 'escape',
    enabled: isOpen,
    handler: () => setOpen(false),
  })

  const handleSelect = (action: CommandPaletteAction) => {
    action.handler()
    setOpen(false)
  }

  return (
    <CommandPaletteOverlay
      isOpen={isOpen}
      onClose={() => setOpen(false)}
      query={query}
      onQueryChange={setQuery}
      actions={filteredActions}
      onSelect={handleSelect}
    />
  )
}
