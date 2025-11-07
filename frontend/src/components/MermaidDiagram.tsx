import { useEffect, useMemo, useState } from 'react'
import mermaid from 'mermaid'

type MermaidConfig = Parameters<typeof mermaid.initialize>[0]

interface MermaidDiagramProps {
  id: string
  chart: string
  config?: MermaidConfig
  ariaLabel?: string
  title?: string
  description?: string
  defaultOpen?: boolean
}

const buildThemeVariables = (mode: 'light' | 'dark') => {
  if (mode === 'dark') {
    return {
      background: 'transparent',
      primaryColor: '#312e81',
      primaryTextColor: '#f8fafc',
      primaryBorderColor: '#6366f1',
      lineColor: '#818cf8',
      secondaryColor: '#0f172a',
      tertiaryColor: '#1f2937',
      fontFamily: '"Inter", "Inter var", system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
      nodeTextColor: '#f8fafc',
      noteTextColor: '#cbd5f5',
      edgeLabelBackground: '#1e293b',
    }
  }

  return {
    background: 'transparent',
    primaryColor: '#e0e7ff',
    primaryTextColor: '#1e293b',
    primaryBorderColor: '#6366f1',
    lineColor: '#475569',
    secondaryColor: '#eef2ff',
    tertiaryColor: '#f8fafc',
    fontFamily: '"Inter", "Inter var", system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
    nodeTextColor: '#1e293b',
    noteTextColor: '#334155',
    edgeLabelBackground: '#e2e8f0',
  }
}

export const MermaidDiagram = ({
  id,
  chart,
  config,
  ariaLabel,
  title,
  description,
  defaultOpen = false,
}: MermaidDiagramProps) => {
  const [hasRendered, setHasRendered] = useState(false)
  const chartDefinition = useMemo(() => chart.trim(), [chart])
  const colorMode = typeof document !== 'undefined' && document.documentElement.classList.contains('dark') ? 'dark' : 'light'

  useEffect(() => {
    if (typeof window === 'undefined') return

    const container = document.getElementById(id)
    if (!container) return

    const renderDiagram = async () => {
      try {
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: 'loose',
          theme: 'base',
          themeVariables: buildThemeVariables(colorMode),
          flowchart: {
            curve: 'basis',
            htmlLabels: true,
          },
          sequence: {
            mirrorActors: false,
          },
          ...config,
        })
        const { svg } = await mermaid.render(`${id}-diagram`, chartDefinition)
        container.innerHTML = svg
        setHasRendered(true)
      } catch (error) {
        console.error('Mermaid render error', error)
      }
    }

    renderDiagram()
  }, [id, chartDefinition, config, colorMode])

  return (
    <details
      className="group rounded-3xl border border-white/60 bg-white/75 p-5 shadow-lg backdrop-blur-xl transition hover:-translate-y-0.5 hover:shadow-xl dark:border-white/10 dark:bg-white/10"
      open={defaultOpen}
    >
      {title && (
        <summary className="flex cursor-pointer list-none items-center justify-between rounded-2xl border border-transparent bg-white/40 px-4 py-3 text-left text-lg font-semibold text-slate-800 outline-none transition hover:border-indigo-200/70 hover:bg-white/70 focus-visible:ring focus-visible:ring-indigo-300 group-open:mb-4 dark:bg-white/5 dark:text-slate-100 dark:hover:border-indigo-500/40 dark:hover:bg-white/10">
          <span>{title}</span>
          <span className="ml-3 flex h-8 w-8 items-center justify-center rounded-full border border-indigo-200 bg-indigo-50 text-sm text-indigo-500 transition-transform group-open:rotate-180 dark:border-indigo-400/40 dark:bg-indigo-500/10 dark:text-indigo-200">
            ▼
          </span>
        </summary>
      )}
      <div className="space-y-4">
        {description && <p className="text-sm leading-relaxed text-slate-600 dark:text-slate-300/90">{description}</p>}
        <div
          id={id}
          role="img"
          aria-label={ariaLabel ?? title ?? 'Architecture diagram'}
          className={`w-full overflow-x-auto rounded-2xl border border-indigo-100/80 bg-white/95 p-0 shadow-inner transition-all duration-300 dark:border-indigo-400/30 dark:bg-slate-950/40 ${hasRendered ? 'group-open:p-6 group-open:min-h-[600px]' : 'animate-pulse'}`}
        />
      </div>
    </details>
  )
}
