export interface CommandPaletteAction {
  id: string
  label: string
  description?: string
  shortcut?: string
  group?: string
  handler: () => void
}
