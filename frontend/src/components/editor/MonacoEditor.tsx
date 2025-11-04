import Editor from '@monaco-editor/react'
import { useCallback, useMemo } from 'react'

import type { PasteFormat } from '../../api/types'
import { useTheme } from '../../theme/ThemeContext'

export interface MonacoEditorProps {
  value: string
  onChange?: (value: string) => void
  format: PasteFormat
  height?: string | number
  readOnly?: boolean
  className?: string
}

const formatToLanguage = (format: PasteFormat): string => {
  switch (format) {
    case 'markdown':
      return 'markdown'
    case 'json':
      return 'json'
    case 'javascript':
      return 'javascript'
    case 'typescript':
      return 'typescript'
    case 'python':
      return 'python'
    case 'rust':
      return 'rust'
    case 'go':
      return 'go'
    case 'cpp':
      return 'cpp'
    case 'kotlin':
      return 'kotlin'
    case 'java':
      return 'java'
    case 'csharp':
      return 'csharp'
    case 'php':
      return 'php'
    case 'ruby':
      return 'ruby'
    case 'bash':
      return 'shell'
    case 'yaml':
      return 'yaml'
    case 'sql':
      return 'sql'
    case 'swift':
      return 'swift'
    case 'html':
      return 'html'
    case 'css':
      return 'css'
    case 'code':
      return 'plaintext'
    case 'plain_text':
    default:
      return 'plaintext'
  }
}

export const MonacoEditor = ({
  value,
  onChange,
  format,
  height = '60vh',
  readOnly = false,
  className,
}: MonacoEditorProps) => {
  const { theme } = useTheme()

  const language = useMemo(() => formatToLanguage(format), [format])
  const handleChange = useCallback(
    (content: string | undefined) => {
      onChange?.(content ?? '')
    },
    [onChange],
  )

  return (
    <Editor
      className={className}
      height={height}
      defaultLanguage="plaintext"
      language={language}
      value={value}
      theme={theme === 'dark' ? 'vs-dark' : 'vs'}
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
        readOnly,
        domReadOnly: readOnly,
      }}
      onChange={handleChange}
    />
  )
}
