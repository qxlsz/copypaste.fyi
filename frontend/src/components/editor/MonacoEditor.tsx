import Editor, { type Monaco } from '@monaco-editor/react'
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
  const editorTheme = useMemo(() => (theme === 'dark' ? 'copypaste-dark' : 'copypaste-light'), [theme])
  const handleChange = useCallback(
    (content: string | undefined) => {
      onChange?.(content ?? '')
    },
    [onChange],
  )
  const handleBeforeMount = useCallback((monaco: Monaco) => {
    monaco.editor.defineTheme('copypaste-dark', {
      base: 'vs-dark',
      inherit: true,
      rules: [],
      colors: {
        'editor.background': 'rgba(0, 0, 0, 0)',
        'editorGutter.background': 'rgba(0, 0, 0, 0)',
        'minimap.background': 'rgba(0, 0, 0, 0)',
      },
    })
    monaco.editor.defineTheme('copypaste-light', {
      base: 'vs',
      inherit: true,
      rules: [],
      colors: {
        'editor.background': 'rgba(255, 255, 255, 0)',
        'editorGutter.background': 'rgba(255, 255, 255, 0)',
        'minimap.background': 'rgba(255, 255, 255, 0)',
      },
    })
  }, [])

  return (
    <Editor
      className={className}
      height={height}
      defaultLanguage="plaintext"
      language={language}
      value={value}
      theme={editorTheme}
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
      beforeMount={handleBeforeMount}
      onChange={handleChange}
    />
  )
}
