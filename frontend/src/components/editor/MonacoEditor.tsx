import Editor from '@monaco-editor/react'
import { useMemo } from 'react'

import type { PasteFormat } from '../../api/types'
import { useTheme } from '../../theme/ThemeContext'

export interface MonacoEditorProps {
  value: string
  onChange: (value: string) => void
  format: PasteFormat
  height?: string | number
}

const formatToLanguage = (format: PasteFormat): string => {
  switch (format) {
    case 'markdown':
      return 'markdown'
    case 'json':
      return 'json'
    case 'go':
      return 'go'
    case 'cpp':
      return 'cpp'
    case 'kotlin':
      return 'kotlin'
    case 'java':
      return 'java'
    case 'code':
      return 'plaintext'
    case 'plain_text':
    default:
      return 'plaintext'
  }
}

export const MonacoEditor = ({ value, onChange, format, height = '60vh' }: MonacoEditorProps) => {
  const { theme } = useTheme()

  const language = useMemo(() => formatToLanguage(format), [format])

  return (
    <Editor
      height={height}
      defaultLanguage="plaintext"
      language={language}
      value={value}
      theme={theme === 'dark' ? 'vs-dark' : 'light'}
      options={{
        fontSize: 14,
        fontFamily: 'JetBrains Mono, ui-monospace, SFMono-Regular',
        minimap: { enabled: false },
        scrollBeyondLastLine: false,
        wordWrap: 'on',
        automaticLayout: true,
        smoothScrolling: true,
        renderWhitespace: 'none',
        tabSize: 2,
      }}
      onChange={(content) => onChange(content ?? '')}
    />
  )
}
