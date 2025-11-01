import { useMemo } from 'react'
import { useParams, useSearchParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'

import { fetchPaste } from '../api/viewer'

const formatLabel = (format: string) => {
  switch (format) {
    case 'plain_text':
      return 'Plain Text'
    case 'markdown':
      return 'Markdown'
    case 'code':
      return 'Code'
    case 'json':
      return 'JSON'
    case 'go':
      return 'Go'
    case 'cpp':
      return 'C++'
    case 'kotlin':
      return 'Kotlin'
    case 'java':
      return 'Java'
    default:
      return format
  }
}

export const PasteViewPage = () => {
  const { id } = useParams<{ id: string }>()
  const [searchParams] = useSearchParams()
  const key = searchParams.get('key') ?? undefined

  const queryKey = useMemo(() => ['paste', id, key], [id, key])

  const { data, isLoading, isError, error } = useQuery({
    enabled: Boolean(id),
    queryKey,
    queryFn: () => fetchPaste(id!, key),
  })

  if (!id) {
    return (
      <div className="mx-auto max-w-3xl space-y-4 text-center">
        <h1 className="text-2xl font-semibold text-slate-100">Paste not found</h1>
        <p className="text-slate-400">The requested paste ID is missing or invalid.</p>
      </div>
    )
  }

  if (isLoading) {
    return <p className="text-slate-400">Loading paste…</p>
  }

  if (isError || !data) {
    const message = error instanceof Error ? error.message : 'Unknown error'
    return (
      <div className="space-y-3">
        <h1 className="text-2xl font-semibold text-danger">Unable to load paste</h1>
        <p className="text-slate-400">{message}</p>
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <header className="space-y-2">
        <h1 className="text-3xl font-semibold text-slate-100">Shared paste</h1>
        <p className="text-slate-400">
          Format: <span className="font-medium text-primary">{formatLabel(data.format)}</span>
        </p>
        <div className="flex flex-wrap gap-4 text-xs text-slate-500">
          <span>Created: {new Date(data.createdAt * 1000).toLocaleString()}</span>
          {data.expiresAt ? <span>Expires: {new Date(data.expiresAt * 1000).toLocaleString()}</span> : <span>Expires: Never</span>}
          {data.burnAfterReading ? <span className="text-danger font-medium">Burn after reading</span> : null}
        </div>
      </header>

      <section className="rounded-2xl border border-slate-800 bg-surface/80 p-6">
        <pre className="overflow-x-auto whitespace-pre-wrap break-words font-mono text-sm text-slate-100">
          {data.content}
        </pre>
      </section>
    </div>
  )
}
