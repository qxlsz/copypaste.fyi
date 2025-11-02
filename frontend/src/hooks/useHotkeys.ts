import { useEffect } from 'react'

export interface HotkeyOptions {
  shortcut: string
  handler: (event: KeyboardEvent) => void
  enabled?: boolean
  preventDefault?: boolean
  target?: HTMLElement | Document | Window
}

const isModifierKey = (value: string) => {
  switch (value) {
    case 'meta':
    case 'cmd':
    case 'command':
    case 'ctrl':
    case 'control':
    case 'shift':
    case 'alt':
    case 'option':
      return true
    default:
      return false
  }
}

const normalizeShortcut = (shortcut: string) => {
  const parts = shortcut
    .split('+')
    .map((part) => part.trim().toLowerCase())
    .filter(Boolean)

  const modifiers = new Set<string>()
  let key: string | null = null

  for (const part of parts) {
    if (isModifierKey(part)) {
      switch (part) {
        case 'cmd':
        case 'command':
          modifiers.add('meta')
          break
        case 'control':
          modifiers.add('ctrl')
          break
        case 'option':
          modifiers.add('alt')
          break
        default:
          modifiers.add(part)
      }
    } else {
      key = part
    }
  }

  return { modifiers, key }
}

const matchesShortcut = (event: KeyboardEvent, shortcut: string) => {
  const { modifiers, key } = normalizeShortcut(shortcut)

  if (modifiers.has('meta') !== event.metaKey) return false
  if (modifiers.has('ctrl') !== event.ctrlKey) return false
  if (modifiers.has('shift') !== event.shiftKey) return false
  if (modifiers.has('alt') !== event.altKey) return false

  if (key === null) {
    return true
  }

  const normalizedKey = key.length === 1 ? key : key.toLowerCase()
  const eventKey = event.key.length === 1 ? event.key.toLowerCase() : event.key.toLowerCase()

  return normalizedKey === eventKey
}

export const useHotkeys = (options: HotkeyOptions | HotkeyOptions[]) => {
  useEffect(() => {
    if (typeof window === 'undefined') {
      return
    }

    const definitions = Array.isArray(options) ? options : [options]
    const filtered = definitions.filter((definition) => definition && definition.enabled !== false)

    if (!filtered.length) {
      return
    }

    const listeners = filtered.map((definition) => {
      const target = definition.target ?? window

      const listener = (event: KeyboardEvent) => {
        if (!matchesShortcut(event, definition.shortcut)) {
          return
        }

        if (definition.preventDefault ?? true) {
          event.preventDefault()
        }

        definition.handler(event)
      }

      target.addEventListener('keydown', listener as EventListener)

      return { target, listener }
    })

    return () => {
      listeners.forEach(({ target, listener }) => {
        target.removeEventListener('keydown', listener as EventListener)
      })
    }
  }, [options])
}
