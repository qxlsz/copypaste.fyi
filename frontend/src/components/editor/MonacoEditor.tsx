import type { Monaco } from '@monaco-editor/react'
import { useCallback, useEffect, useMemo, useState } from 'react'
import clsx from 'clsx'

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

type EditorModule = typeof import('@monaco-editor/react')

export const MonacoEditor = ({
  value,
  onChange,
  format,
  height = '60vh',
  readOnly = false,
  className,
}: MonacoEditorProps) => {
  const { theme } = useTheme()
  const [editorModule, setEditorModule] = useState<EditorModule | null>(null)
  const [isMobile, setIsMobile] = useState(false)

  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
      return
    }

    const mediaQuery = window.matchMedia('(max-width: 768px)')

    const updateMobile = (event: MediaQueryList | MediaQueryListEvent) => {
      setIsMobile(event.matches)
    }

    updateMobile(mediaQuery)

    if (typeof mediaQuery.addEventListener === 'function') {
      mediaQuery.addEventListener('change', updateMobile)
    } else if (typeof mediaQuery.addListener === 'function') {
      mediaQuery.addListener(updateMobile)
    }

    return () => {
      if (typeof mediaQuery.removeEventListener === 'function') {
        mediaQuery.removeEventListener('change', updateMobile)
      } else if (typeof mediaQuery.removeListener === 'function') {
        mediaQuery.removeListener(updateMobile)
      }
    }
  }, [])

  useEffect(() => {
    if (isMobile) {
      setEditorModule(null)
      return
    }

    let cancelled = false
    import('@monaco-editor/react')
      .then((module) => {
        if (!cancelled) {
          setEditorModule(module)
        }
      })
      .catch(() => {
        setEditorModule(null)
      })
    return () => {
      cancelled = true
    }
  }, [isMobile])

  const language = useMemo(() => formatToLanguage(format), [format])
  const editorTheme = useMemo(() => (theme === 'dark' ? 'copypaste-dark' : 'copypaste-light'), [theme])
  const handleChange = useCallback(
    (content: string | undefined) => {
      onChange?.(content ?? '')
    },
    [onChange],
  )
  const resolvedHeight = useMemo(() => {
    if (isMobile) {
      return '45vh'
    }
    return height
  }, [height, isMobile])
  const handleBeforeMount = useCallback((monaco: Monaco) => {
    monaco.editor.defineTheme('copypaste-dark', {
      base: 'vs-dark',
      inherit: true,
      rules: [],
      colors: {
        'editor.background': '#00000000',
        'editorGutter.background': '#00000000',
        'minimap.background': '#00000000',
      },
    })
    monaco.editor.defineTheme('copypaste-light', {
      base: 'vs',
      inherit: true,
      rules: [],
      colors: {
        'editor.background': '#FFFFFF00',
        'editorGutter.background': '#FFFFFF00',
        'minimap.background': '#FFFFFF00',
      },
    })
  }, [])

  if (!editorModule) {
    if (readOnly) {
      return (
        <pre
          className={clsx(
            'overflow-x-auto overflow-y-auto whitespace-pre-wrap break-words font-mono text-sm',
            theme === 'dark'
              ? 'rounded-xl border border-slate-700 bg-surface/80 p-4 text-slate-100'
              : 'rounded-xl border border-slate-200 bg-white p-4 text-slate-800',
            className,
          )}
          style={{ minHeight: resolvedHeight, maxHeight: resolvedHeight, height: resolvedHeight }}
        >
          {value}
        </pre>
      )
    }

    return (
      <textarea
        value={value}
        onChange={(event) => onChange?.(event.target.value)}
        className={clsx(
          'w-full rounded-2xl border p-4 font-mono text-sm transition focus:border-primary focus:outline-none focus:ring focus:ring-primary/20',
          theme === 'dark'
            ? 'border-slate-700 bg-surface text-slate-100'
            : 'border-slate-200 bg-white text-slate-900',
          className,
        )}
        style={{ minHeight: resolvedHeight, height: resolvedHeight }}
      />
    )
  }

  const EditorComponent = editorModule.default

  return (
    <EditorComponent
      className={className}
      height={resolvedHeight}
      defaultLanguage="plaintext"
      language={language}
      value={value}
      theme={editorTheme}
      options={{
        fontSize: 14,
        fontFamily: 'JetBrains Mono, ui-monospace, SFMono-Regular',
        minimap: { enabled: false },
        lineNumbers: 'off',
        glyphMargin: false,
        folding: false,
        renderLineHighlight: 'none',
        overviewRulerBorder: false,
        overviewRulerLanes: 0,
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
